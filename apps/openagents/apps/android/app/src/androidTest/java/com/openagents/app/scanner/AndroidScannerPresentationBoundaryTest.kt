package com.openagents.app.scanner

import android.test.InstrumentationTestCase
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import java.nio.ByteBuffer

/** Synthetic camera results exercise permission, bounds, retry, and duplicate fencing. */
class AndroidScannerPresentationBoundaryTest : InstrumentationTestCase() {
    fun testDeniedPermissionCanRetryAndOnlySubmitsRawTextToRust() {
        val classified = mutableListOf<String>()
        val scanner = AndroidScannerPresentationBoundary(classifyWithRust = { classified.add(it) })
        val deniedRequest = scanner.requestPermission()!!

        assertEquals(AndroidScanResult.NOT_SCANNING, scanner.onScanText(deniedRequest, FakeCameraScan.CONNECT))
        assertTrue(scanner.permissionDenied(deniedRequest))
        assertEquals(AndroidScannerState.PERMISSION_DENIED, scanner.state)
        assertEquals(AndroidScanResult.STALE_SESSION, scanner.onScanText(deniedRequest, FakeCameraScan.CONNECT))
        assertEquals(emptyList<String>(), classified)

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

    fun testZxingDecodesBoundedCameraYPlaneWithoutNormalizingText() {
        val expected = "https://openagents.com/connect#synthetic-qr"
        val matrix = QRCodeWriter().encode(expected, BarcodeFormat.QR_CODE, 256, 256)
        val rowStride = matrix.width * 2 + 9
        val pixelStride = 2
        val bufferOffset = 5
        val yPlane = ByteBuffer.allocate(bufferOffset + rowStride * matrix.height)
        yPlane.position(bufferOffset)
        for (y in 0 until matrix.height) {
            for (x in 0 until matrix.width) {
                val luminance = if (matrix.get(x, y)) 0.toByte() else 0xff.toByte()
                yPlane.put(bufferOffset + y * rowStride + x * pixelStride, luminance)
            }
        }

        val decoder = ZxingQrDecoder()
        assertEquals(
            expected,
            decoder.decode(yPlane, matrix.width, matrix.height, rowStride, pixelStride, rotationDegrees = 0),
        )
        decoder.close()
        assertNull(decoder.decode(yPlane, matrix.width, matrix.height, rowStride, pixelStride, rotationDegrees = 0))
    }

    fun testZxingAppliesCameraRotationBeforeDecoding() {
        val expected = "https://openagents.com/projects/join/synthetic-qr"
        val matrix = QRCodeWriter().encode(expected, BarcodeFormat.QR_CODE, 256, 256)
        val upright = cameraYPlane(matrix)
        val rawCounterClockwise = ByteArray(upright.size)
        for (y in 0 until matrix.height) {
            for (x in 0 until matrix.width) {
                rawCounterClockwise[(matrix.height - 1 - x) * matrix.width + y] = upright[y * matrix.width + x]
            }
        }

        val decoder = ZxingQrDecoder()
        assertEquals(
            expected,
            decoder.decode(
                ByteBuffer.wrap(rawCounterClockwise),
                matrix.width,
                matrix.height,
                rowStride = matrix.width,
                pixelStride = 1,
                rotationDegrees = 90,
            ),
        )
        decoder.close()
    }

    fun testZxingRejectsUnboundedOrMalformedCameraFrames() {
        val decoder = ZxingQrDecoder()
        val frame = ByteBuffer.allocate(8)

        assertNull(decoder.decode(frame, 1281, 1, 1281, 1, rotationDegrees = 0))
        assertNull(decoder.decode(frame, 2, 2, 2, 1, rotationDegrees = 45))
        assertNull(decoder.decode(frame, 4, 2, 3, 1, rotationDegrees = 0))
        decoder.close()
    }

    private fun cameraYPlane(matrix: com.google.zxing.common.BitMatrix): ByteArray =
        ByteArray(matrix.width * matrix.height) { index ->
            val x = index % matrix.width
            val y = index / matrix.width
            if (matrix.get(x, y)) 0.toByte() else 0xff.toByte()
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
