package com.openagents.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.camera.view.PreviewView
import com.openagents.app.scanner.AndroidScannerPresentationBoundary
import com.openagents.app.scanner.CameraXQrScannerDriver

class MainActivity : ComponentActivity() {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var credentialResult: ((Boolean) -> Unit)? = null
    private var credentialPending = false
    private val credentialLauncher = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val callback = credentialResult
        credentialResult = null; credentialPending = false
        callback?.invoke(result.resultCode == RESULT_OK)
    }

    internal fun launchDeviceCredential(intent: android.content.Intent, result: (Boolean) -> Unit): AutoCloseable {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (credentialPending) { result(false); return AutoCloseable {} }
        credentialPending = true; credentialResult = result
        try { credentialLauncher.launch(intent) }
        catch (_: Exception) { credentialPending = false; credentialResult = null; result(false) }
        return AutoCloseable { if (credentialResult === result) credentialResult = null }
    }
    private var rootPresentation: AndroidRootPresentation? = null
    private var attachmentResults: AndroidAttachmentActivityResultOwner? = null
    private var attachmentResultListener: ((AndroidAttachmentResult) -> Unit)? = null
    private val scannerDrivers = mutableListOf<CameraXQrScannerDriver>()
    private var scannerPresentation: AndroidScannerPresentationBoundary? = null
    private var scannerPreview: PreviewView? = null
    private var scannerSession: Long? = null
    private var scannerPermissionPending = false
    private var scannerFailure: ((Exception) -> Unit)? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        attachmentResults = AndroidAttachmentActivityResultOwner(this) { result ->
            if (result is AndroidAttachmentResult.CameraPermission && handleScannerPermission(result.granted)) {
                return@AndroidAttachmentActivityResultOwner
            }
            attachmentResultListener?.invoke(result)
        }
        window.statusBarColor = getColor(R.color.oa_background)
        window.navigationBarColor = getColor(R.color.oa_background)
        val debugLaunch = DebugLaunchExtras.read(intent)
        val app = application as OpenAgentsApplication
        rootPresentation = AndroidRootPresentation(this, app, app.androidHost(BuildConfig.DEBUG && debugLaunch.demo))
        setContentView(requireNotNull(rootPresentation).view())
        onBackPressedDispatcher.addCallback(this, object : androidx.activity.OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (rootPresentation?.handleBack() != true) {
                    isEnabled = false
                    onBackPressedDispatcher.onBackPressed()
                    isEnabled = true
                }
            }
        })
    }

    override fun onConfigurationChanged(configuration: android.content.res.Configuration) {
        super.onConfigurationChanged(configuration)
        rootPresentation?.configurationChanged()
    }

    override fun onPause() {
        rootPresentation?.background()
        super.onPause()
    }
    override fun onResume() {
        super.onResume()
        rootPresentation?.foreground()
    }

    override fun onDestroy() {
        credentialResult?.invoke(false); credentialResult = null
        rootPresentation?.close()
        rootPresentation = null
        mainHandler.removeCallbacksAndMessages(null)
        attachmentResults?.close()
        attachmentResults = null
        attachmentResultListener = null
        scannerDrivers.forEach(CameraXQrScannerDriver::close)
        scannerDrivers.clear()
        scannerPresentation?.close()
        scannerPresentation = null
        scannerPreview = null
        scannerSession = null
        scannerPermissionPending = false
        scannerFailure = null
        super.onDestroy()
    }

    /** Installs the current screen's bounded URI intake callback. */
    fun setAttachmentResultListener(listener: ((AndroidAttachmentResult) -> Unit)?) {
        check(Looper.myLooper() == Looper.getMainLooper())
        attachmentResultListener = listener
    }

    fun pickPhotoAttachment() {
        attachmentResults?.pickPhoto()
    }

    fun pickDocumentAttachment(mimeTypes: Array<String>) {
        attachmentResults?.pickDocument(mimeTypes)
    }

    fun captureAttachment(output: android.net.Uri) {
        attachmentResults?.capturePhoto(output)
    }

    fun requestCameraPermission() {
        attachmentResults?.requestCameraPermission()
    }

    /** Starts QR capture and sends the decoded value to the supplied Rust classifier. */
    fun startQrScanner(
        previewView: PreviewView,
        classifyWithRust: (String) -> Unit,
        onFailure: (Exception) -> Unit,
    ): Boolean {
        if (scannerPresentation != null) return false
        val presentation = AndroidScannerPresentationBoundary(classifyWithRust = { rawText ->
            try {
                classifyWithRust(rawText)
            } finally {
                scannerSession = null
                scannerPresentation = null
                scannerPreview = null
            }
        })
        val session = presentation.requestPermission() ?: return false
        scannerPresentation = presentation
        scannerPreview = previewView
        scannerFailure = onFailure
        scannerSession = session
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            startScannerCapture(presentation, session)
        } else {
            scannerPermissionPending = true
            if (attachmentResults == null) {
                scannerPermissionPending = false
                presentation.permissionDenied(session)
                scannerFailure?.invoke(IllegalStateException("Camera permission is unavailable."))
                clearScannerRequest()
                return false
            }
            attachmentResults?.requestCameraPermission()
        }
        return true
    }

    /** Retries a denied, cancelled, or failed scan after the caller asks the person to retry. */
    fun retryQrScanner(): Boolean {
        val presentation = scannerPresentation ?: return false
        val preview = scannerPreview ?: return false
        scannerDrivers.lastOrNull()?.cancel()
        val session = presentation.retry() ?: return false
        scannerSession = session
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            startScannerCapture(presentation, session, preview)
        } else {
            scannerPermissionPending = true
            if (attachmentResults == null) {
                scannerPermissionPending = false
                return false
            }
            attachmentResults?.requestCameraPermission()
        }
        return true
    }

    /** Cancels the current permission request or camera session. */
    fun cancelQrScanner() {
        val session = scannerSession ?: return
        scannerPresentation?.cancel(session)
        scannerDrivers.lastOrNull()?.cancel()
        scannerSession = null
    }

    private fun handleScannerPermission(granted: Boolean): Boolean {
        if (!scannerPermissionPending) return false
        scannerPermissionPending = false
        val session = scannerSession ?: return true
        val presentation = scannerPresentation ?: return false
        if (granted) {
            startScannerCapture(presentation, session)
        } else {
            presentation.permissionDenied(session)
            scannerFailure?.invoke(SecurityException("Camera permission was denied."))
            scannerSession = null
        }
        return true
    }

    private fun startScannerCapture(
        presentation: AndroidScannerPresentationBoundary,
        session: Long,
        preview: PreviewView = checkNotNull(scannerPreview),
    ) {
        if (!presentation.permissionGranted(session)) return
        val driver = CameraXQrScannerDriver(
            this,
            this,
            preview,
            presentation,
            session,
        ) { error ->
            scannerSession = null
            scannerFailure?.invoke(error)
        }
        scannerDrivers += driver
        if (!driver.start()) {
            scannerSession = null
            if (presentation.captureFailed(session)) {
                scannerFailure?.invoke(IllegalStateException("Could not start the camera preview."))
            }
        }
    }

    private fun clearScannerRequest() {
        scannerSession = null
        scannerPermissionPending = false
        scannerPresentation = null
        scannerPreview = null
    }

}
