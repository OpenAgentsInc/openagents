package com.openagents.app.scanner

import android.test.InstrumentationTestCase

/** Synthetic camera results exercise permission, bounds, retry, and duplicate fencing. */
class AndroidScannerPresentationBoundaryTest : InstrumentationTestCase() {
    fun testDeniedPermissionCanRetryAndOnlySubmitsRawTextToRust() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val deniedRequest = scanner.requestPermission()!!

        assertTrue(scanner.permissionDenied(deniedRequest))
        assertEquals(AndroidScannerState.PERMISSION_DENIED, scanner.state)
        assertEquals(AndroidScanResult.NOT_SCANNING, scanner.onScanText(deniedRequest, FakeCameraScan.CONNECT))

        val retryRequest = scanner.retry()!!
        assertFalse(scanner.permissionGranted(deniedRequest))
        assertTrue(scanner.permissionGranted(retryRequest))
        assertEquals(AndroidScanResult.SUBMITTED_TO_RUST, scanner.onScanText(retryRequest, FakeCameraScan.CONNECT))
        assertEquals(listOf(FakeCameraScan.CONNECT), classified)
    }

    fun testCancelRetryAndLateCameraCallbackAreFenced() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val request = scanner.requestPermission()!!

        assertTrue(scanner.cancel(request))
        assertEquals(AndroidScannerState.CANCELLED, scanner.state)
        assertEquals(AndroidScanResult.STALE_SESSION, scanner.onScanText(request, FakeCameraScan.PROJECT_INVITATION))

        val retry = scanner.retry()!!
        assertTrue(scanner.permissionGranted(retry))
        assertEquals(AndroidScanResult.SUBMITTED_TO_RUST, scanner.onScanText(retry, FakeCameraScan.PROJECT_INVITATION))
        assertEquals(listOf(FakeCameraScan.PROJECT_INVITATION), classified)
    }

    fun testCameraFailureOffersPermissionRetryWithoutClassifying() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val request = scanner.requestPermission()!!
        assertTrue(scanner.permissionGranted(request))

        assertTrue(scanner.captureFailed(request))
        assertEquals(AndroidScannerState.CAMERA_ERROR, scanner.state)
        assertEquals(AndroidScanResult.STALE_SESSION, scanner.onScanText(request, FakeCameraScan.CONNECT))
        val retry = scanner.retry()!!
        assertTrue(scanner.permissionGranted(retry))
        assertEquals(emptyList<String>(), classified)
    }

    fun testOversizedTextAndDuplicateFramesNeverReachClassifierTwice() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val request = scanner.requestPermission()!!
        assertTrue(scanner.permissionGranted(request))

        assertEquals(AndroidScanResult.TOO_LARGE, scanner.onScanText(request, FakeCameraScan.OVERSIZED))
        assertEquals(AndroidScanResult.SUBMITTED_TO_RUST, scanner.onScanText(request, FakeCameraScan.PROJECT_INVITATION))
        assertEquals(AndroidScanResult.DUPLICATE, scanner.onScanText(request, FakeCameraScan.CONNECT))
        assertEquals(listOf(FakeCameraScan.PROJECT_INVITATION), classified)
    }

    fun testCloseFencesPendingPermissionAndCameraCallbacks() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val request = scanner.requestPermission()!!
        scanner.close()

        assertFalse(scanner.permissionGranted(request))
        assertEquals(AndroidScannerState.CLOSED, scanner.state)
        assertEquals(AndroidScanResult.STALE_SESSION, scanner.onScanText(request, FakeCameraScan.CONNECT))
        assertEquals(emptyList<String>(), classified)
    }

    fun testFakeImageAnalysisDecoderFeedsOneRawValueAndFencesLateFrames() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val request = scanner.requestPermission()!!
        assertTrue(scanner.permissionGranted(request))
        var stopCount = 0
        val dispatcher = ScannerDecodedTextDispatcher(scanner, request) { stopCount += 1 }
        val analyzer = FakeImageAnalysis(FakeQrDecoder(FakeCameraScan.frames), dispatcher)

        assertNull(analyzer.analyze("blank-frame"))
        assertEquals(AndroidScanResult.SUBMITTED_TO_RUST, analyzer.analyze("project-qr-frame"))
        assertEquals(AndroidScanResult.STALE_SESSION, analyzer.analyze("duplicate-project-qr-frame"))
        analyzer.close()
        assertEquals(AndroidScanResult.STALE_SESSION, analyzer.analyze("late-project-qr-frame"))
        assertEquals(listOf(FakeCameraScan.PROJECT_INVITATION), classified)
        assertEquals(1, stopCount)
    }

    private class FakeQrDecoder(private val frames: Map<String, String?>) {
        fun decode(frame: String): String? = frames[frame]
    }

    private class FakeImageAnalysis(
        private val decoder: FakeQrDecoder,
        private val dispatcher: ScannerDecodedTextDispatcher,
    ) {
        private var closed = false

        fun analyze(frame: String): AndroidScanResult? =
            if (closed) AndroidScanResult.STALE_SESSION else dispatcher.onDecoded(decoder.decode(frame))

        fun close() {
            closed = true
            dispatcher.close()
        }
    }

    private object FakeCameraScan {
        const val CONNECT = "https://openagents.com/connect#synthetic-camera-code"
        const val PROJECT_INVITATION = "https://openagents.com/projects/join/prjinv_fake-camera-01"
        val OVERSIZED = "x".repeat(2049)
        val frames = mapOf(
            "blank-frame" to null,
            "project-qr-frame" to PROJECT_INVITATION,
            "duplicate-project-qr-frame" to PROJECT_INVITATION,
            "late-project-qr-frame" to PROJECT_INVITATION,
        )
    }
}
