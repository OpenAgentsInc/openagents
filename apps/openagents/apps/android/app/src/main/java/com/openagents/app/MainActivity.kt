package com.openagents.app

import android.graphics.Typeface
import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.camera.view.PreviewView
import uniffi.openagents_core.CloudChatListener
import uniffi.openagents_core.CloudChatSnapshot
import uniffi.openagents_core.CoreCloudChat
import com.openagents.app.scanner.AndroidScannerPresentationBoundary
import com.openagents.app.scanner.CameraXQrScannerDriver

class MainActivity : ComponentActivity() {
    private val mainHandler = Handler(Looper.getMainLooper())
    private lateinit var transcript: LinearLayout
    private lateinit var input: EditText
    private lateinit var send: Button
    private var demoChat: CoreCloudChat? = null
    private var transcriptScroll: ScrollView? = null
    private var attachmentResults: AndroidAttachmentActivityResultOwner? = null
    private var attachmentResultListener: ((AndroidAttachmentResult) -> Unit)? = null
    private val scannerDrivers = mutableListOf<CameraXQrScannerDriver>()
    private var scannerPresentation: AndroidScannerPresentationBoundary? = null
    private var scannerPreview: PreviewView? = null
    private var scannerSession: Long? = null
    private var scannerPermissionPending = false
    private var scannerFailure: ((Exception) -> Unit)? = null

    private val listener = object : CloudChatListener {
        override fun onCloudChatChanged(snapshot: CloudChatSnapshot) {
            mainHandler.post {
                if (!isFinishing && !isDestroyed) render(snapshot)
            }
        }
    }

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
        setContentView(createScreen(debugLaunch))

        if (BuildConfig.DEBUG && debugLaunch.demo) {
            // The first shell screen is an offline Rust demo. It has no
            // account, network client, or saved conversation.
            demoChat = CoreCloudChat.demo(listener)
            demoChat?.snapshot()?.let(::render)
        }
    }

    override fun onDestroy() {
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
        demoChat?.close()
        demoChat = null
        transcriptScroll = null
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

    private fun createScreen(debugLaunch: DebugLaunchExtras): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(getColor(R.color.oa_background))
            setPadding(dp(24), dp(20), dp(24), dp(18))
        }
        if (BuildConfig.DEBUG) root.tag = "route=${debugLaunch.route};lab=${debugLaunch.lab}"
        root.addView(TextView(this).apply {
            text = getString(R.string.demo_title)
            setTextColor(getColor(R.color.oa_text))
            textSize = 26f
            typeface = face("Geist-SemiBold.ttf", Typeface.create("sans-serif-medium", Typeface.NORMAL))
        })
        if (BuildConfig.DEBUG && debugLaunch.demo) root.addView(TextView(this).apply {
            text = getString(R.string.demo_subtitle)
            setTextColor(getColor(R.color.oa_secondary))
            textSize = 15f
            typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
            setPadding(0, dp(5), 0, dp(12))
        })

        transcript = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, dp(12), 0, dp(12))
        }
        transcriptScroll = ScrollView(this).apply {
            isFillViewport = true
            addView(transcript)
        }
        root.addView(transcriptScroll, LinearLayout.LayoutParams(0, 0, 1f))

        val composer = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(4), dp(4), dp(4))
            setBackgroundColor(getColor(R.color.oa_surface))
        }
        input = EditText(this).apply {
            hint = getString(R.string.demo_message_hint)
            setTextColor(getColor(R.color.oa_text))
            setHintTextColor(getColor(R.color.oa_secondary))
            textSize = 16f
            typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
            setSingleLine(true)
            imeOptions = EditorInfo.IME_ACTION_SEND
            background = null
            setOnEditorActionListener { _, action, _ ->
                if (action == EditorInfo.IME_ACTION_SEND) { sendMessage(); true } else false
            }
        }
        composer.addView(input, LinearLayout.LayoutParams(0, dp(48), 1f))
        send = Button(this).apply {
            text = getString(R.string.demo_send)
            setTextColor(getColor(R.color.oa_background))
            setBackgroundColor(getColor(R.color.oa_accent))
            typeface = face("Geist-Medium.ttf", Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setOnClickListener { sendMessage() }
        }
        composer.addView(send, LinearLayout.LayoutParams(dp(76), dp(44)))
        if (BuildConfig.DEBUG && debugLaunch.demo) root.addView(composer)
        return root
    }

    private fun render(snapshot: CloudChatSnapshot) {
        transcript.removeAllViews()
        snapshot.lines.forEach { line ->
            val row = TextView(this).apply {
                text = line.text
                textSize = 16f
                setTextColor(getColor(R.color.oa_text))
                typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
                setPadding(dp(14), dp(12), dp(14), dp(12))
                setBackgroundColor(if (line.role.name == "USER") getColor(R.color.oa_surface) else getColor(R.color.oa_background))
            }
            transcript.addView(row, LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT)
        }
        if (snapshot.lines.isEmpty()) {
            transcript.addView(TextView(this).apply {
                text = "Try a question. This demo works without an internet connection."
                textSize = 16f
                setTextColor(getColor(R.color.oa_secondary))
                typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
                setPadding(dp(4), dp(12), dp(4), dp(12))
            })
        }
        send.isEnabled = !snapshot.answering
        transcriptScroll?.post { transcriptScroll?.fullScroll(View.FOCUS_DOWN) }
    }

    private fun sendMessage() {
        val text = input.text.toString().trim()
        if (text.isEmpty() || demoChat == null) return
        input.text.clear()
        demoChat?.send(text)
    }

    private fun face(asset: String, fallback: Typeface): Typeface =
        runCatching { Typeface.createFromAsset(assets, asset) }.getOrDefault(fallback)

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
}
