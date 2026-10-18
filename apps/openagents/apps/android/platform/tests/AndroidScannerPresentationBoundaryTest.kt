package com.openagents.app

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

    private object FakeCameraScan {
        const val CONNECT = "https://openagents.com/connect#synthetic-camera-code"
        const val PROJECT_INVITATION = "https://openagents.com/projects/join/prjinv_fake-camera-01"
        val OVERSIZED = "x".repeat(2049)
    }
}
