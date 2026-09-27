package com.openagents.coder

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONObject

/** Thin native mounting, input, lifecycle, and protected storage adapters. */
class MainActivity : ComponentActivity() {
    private lateinit var root: FrameLayout
    private lateinit var safe: FrameLayout
    private lateinit var world: VerseSurface
    private lateinit var reader: ReaderBridge
    private lateinit var scanner: QRScanner
    private lateinit var renderer: NativeRenderer
    private lateinit var gym: GymPanel
    private lateinit var status: TextView
    private lateinit var worldError: TextView
    private lateinit var retry: Button
    private lateinit var controls: LinearLayout
    private lateinit var cameraButton: Button
    private lateinit var recenter: Button
    private lateinit var motionStatus: TextView
    private lateinit var computerButton: Button
    private lateinit var gymButton: Button
    private lateinit var diagnostics: LinearLayout
    private lateinit var panel: LinearLayout
    private lateinit var panelBody: LinearLayout
    private var readerContent: LinearLayout? = null
    private var readerError: TextView? = null
    private var readerStatus: TextView? = null
    private var scannerContainer: LinearLayout? = null
    private var computerMode = ""
    private var opened = ""
    private var pairing = false
    private var details = false
    private var worldDetails = false
    private var scanning = false
    private var requestingCamera = false
    private var foreground = false
    private var synthetic = false
    private var latestWorld: JSONObject? = null
    private var gymBoard: JSONObject? = null
    private var requestedGymRevision = -1L
    private var mountedGymRevision = -1L
    private var lastReading = false
    private val main = Handler(Looper.getMainLooper())
    private val refresh = object : Runnable {
        override fun run() {
            if (foreground && opened == "computer" && !pairing && !scanning) reader.refresh()
            main.postDelayed(this, 5000)
        }
    }
    private val cameraPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { allowed ->
        requestingCamera = false
        if (scanning && opened == "computer") {
            if (allowed) { if (foreground) startCamera() } else cameraFailure("Camera access is off. Enable it in Settings, or paste the invitation.")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        synthetic = BuildConfig.DEBUG && intent.getBooleanExtra("synthetic", false)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = Color.TRANSPARENT; window.navigationBarColor = Color.TRANSPARENT
        if (android.os.Build.VERSION.SDK_INT >= 29) window.isNavigationBarContrastEnforced = false
        if (android.os.Build.VERSION.SDK_INT >= 28) window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = false; isAppearanceLightNavigationBars = false
        }
        val storage = DeviceStorage(this, synthetic)
        root = FrameLayout(this).apply { setBackgroundColor(BACKGROUND) }
        safe = FrameLayout(this)
        scanner = QRScanner(this)
        world = VerseSurface(this, storage, synthetic, intent.getBooleanExtra("gym_preview", false),
            synthetic && intent.getBooleanExtra("motion_preview", false)) { value, error -> receiveWorld(value, error) }
        root.addView(world, FrameLayout.LayoutParams(-1, -1))
        root.addView(safe, FrameLayout.LayoutParams(-1, -1))
        createOverlay()
        renderer = NativeRenderer(this, { view, node ->
            reader.request(json("op" to "activate", "instance" to view.getString("instance"),
                "revision" to view.getLong("revision"), "node" to node))
        }, { enabled, page -> reader.follow(enabled, page) })
        gym = GymPanel(this, world)
        reader = ReaderBridge(storage, synthetic) { if (opened == "computer") renderComputer() }
        setContentView(root)
        ViewCompat.setOnApplyWindowInsetsListener(safe) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            view.setPadding(bars.left + dp(12), bars.top + dp(12), bars.right + dp(12), maxOf(bars.bottom, keyboard.bottom) + dp(12))
            insets
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (opened.isNotEmpty()) closePanel() else { isEnabled = false; onBackPressedDispatcher.onBackPressed(); isEnabled = true }
            }
        })
        main.post(refresh)
    }

    private fun createOverlay() {
        val header = column()
        header.addView(label("Coder", size = 19f))
        status = label("Opening world", "verse-status", 12f); header.addView(status)
        worldError = label("", "verse-error").apply { visibility = View.GONE }; header.addView(worldError)
        retry = button("Retry world renderer", "verse-retry") { world.retry() }.apply { visibility = View.GONE }; header.addView(retry)
        safe.addView(header, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
        controls = column()
        diagnostics = column()
        if (synthetic) {
            for (id in listOf("verse-frames", "verse-position", "gym-interest", "verse-camera", "verse-motion-needed")) diagnostics.addView(label("", id, 11f))
            if (intent.getBooleanExtra("motion_preview", false)) diagnostics.addView(button("Inject motion sample", "verse-motion-sample") { world.injectMotion() })
            controls.addView(diagnostics)
        }
        controls.addView(label("Walk to the computer to connect your chats.", size = 12f))
        controls.addView(label("Drag left to move · drag right to look", "verse-controls-hint", 11f))
        val modes = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        cameraButton = button("Touch look", "verse-camera-mode") { world.toggleMotion() }
        modes.addView(cameraButton, LinearLayout.LayoutParams(0, -2, 1f))
        recenter = button("Recenter", "verse-motion-recenter") { world.send(json("action" to "reset_motion")) }
        modes.addView(recenter); controls.addView(modes)
        motionStatus = label("", "verse-motion-error", 11f); controls.addView(motionStatus)
        val movement = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        movement.addView(button("Jump", "verse-jump") { world.send(json("action" to "jump")) }, LinearLayout.LayoutParams(0, -2, 1f))
        movement.addView(Switch(this).apply {
            text = "Sprint"; tag = "verse-sprint"; setTextColor(AMBER)
            setOnCheckedChangeListener { _, checked -> world.send(json("action" to "sprint", "enabled" to checked)) }
        })
        movement.addView(button("+", "verse-zoom-in") { world.send(json("action" to "zoom", "delta" to 1)) }.apply { contentDescription = "Zoom in" }, LinearLayout.LayoutParams(dp(48), -2))
        movement.addView(button("−", "verse-zoom-out") { world.send(json("action" to "zoom", "delta" to -1)) }.apply { contentDescription = "Zoom out" }, LinearLayout.LayoutParams(dp(48), -2))
        controls.addView(movement)
        safe.addView(controls, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
        computerButton = button("Computer", "computer-interact") { world.send(json("action" to "interact_computer")) }.apply { visibility = View.GONE }
        gymButton = button("Gym board", "gym-interact") { world.send(json("action" to "interact_gym")) }.apply { visibility = View.GONE }
        root.addView(computerButton, FrameLayout.LayoutParams(dp(180), -2))
        root.addView(gymButton, FrameLayout.LayoutParams(dp(190), -2))
        panel = column().apply {
            visibility = View.GONE
            setPadding(dp(12), dp(10), dp(12), dp(10))
            background = GradientDrawable().apply { setColor(0xfa060500.toInt()); cornerRadius = dp(16).toFloat(); setStroke(dp(1), AMBER) }
            isClickable = true
        }
        safe.addView(panel, FrameLayout.LayoutParams(-1, -1).apply { topMargin = dp(60) })
    }

    private fun receiveWorld(value: JSONObject?, error: String?) {
        if (!::status.isInitialized) return
        if (value != null) latestWorld = value
        val packet = latestWorld
        status.text = packet?.optString("status") ?: "Opening world"
        val problem = error ?: packet?.textOrNull("error")
        worldError.text = problem.orEmpty(); worldError.visibility = if (problem == null) View.GONE else View.VISIBLE
        retry.visibility = worldError.visibility
        if (packet == null) return
        val newPanel = when { packet.optBoolean("computer_open") -> "computer"; packet.optBoolean("gym_open") -> "gym"; else -> "" }
        if (newPanel != opened) {
            opened = newPanel; computerMode = ""; renderer.clear(); stopCamera(); controls.visibility = if (opened.isEmpty()) View.VISIBLE else View.GONE
            controls.findViewWithTag<Switch>("verse-sprint")?.isChecked = false
            reader.foreground(foreground && opened == "computer")
            if (opened.isEmpty()) panel.visibility = View.GONE else {
                panel.visibility = View.VISIBLE; panel.removeAllViews()
                val header = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
                header.addView(label(if (opened == "computer") "Computer" else "Gym", size = 18f), LinearLayout.LayoutParams(0, -2, 1f))
                header.addView(button("Back to world", if (opened == "computer") "computer-close" else "gym-close") { closePanel() })
                panel.addView(header)
                panelBody = column(); panel.addView(panelBody, LinearLayout.LayoutParams(-1, 0, 1f))
                if (opened == "computer") renderComputer(true) else mountedGymRevision = -1
            }
        }
        placeAnchor(computerButton, packet.getJSONObject("computer"), opened.isEmpty())
        val location = packet.getJSONObject("gym")
        placeAnchor(gymButton, location, opened.isEmpty() && location.optBoolean("inside"))
        computerButton.text = if (packet.getJSONObject("computer").optBoolean("near")) "Use computer" else "Computer"
        gymButton.text = if (location.optBoolean("near")) "Open Gym board" else "Gym board"
        cameraButton.text = if (packet.optString("camera_mode") == "motion") "Motion look" else "Touch look"
        cameraButton.isEnabled = foreground && world.motionAvailable
        recenter.visibility = if (packet.optString("camera_mode") == "motion") View.VISIBLE else View.GONE
        motionStatus.text = world.motionError ?: if (!world.motionAvailable) "Motion look is unavailable on this device." else ""
        controls.findViewWithTag<TextView>("verse-controls-hint").text = if (packet.optString("camera_mode") == "motion") "Hold left to move · turn your phone to look" else "Drag left to move · drag right to look"
        if (synthetic) {
            diagnostics.findViewWithTag<TextView>("verse-frames").text = "Frames ${packet.optLong("frames_presented")}"
            diagnostics.findViewWithTag<TextView>("verse-position").text = (0..2).joinToString(", ") { "%.2f".format(java.util.Locale.US, packet.getJSONArray("position").getDouble(it)) }
            diagnostics.findViewWithTag<TextView>("gym-interest").text = if (packet.optBoolean("gym_active")) "Gym listening" else "Gym idle"
            diagnostics.findViewWithTag<TextView>("verse-camera").text = "%.5f, %.5f".format(java.util.Locale.US, packet.optDouble("camera_yaw"), packet.optDouble("camera_pitch"))
            diagnostics.findViewWithTag<TextView>("verse-motion-needed").text = if (packet.optBoolean("motion_needed")) "Motion active" else "Motion idle"
        }
        if (!packet.optBoolean("gym_active") || !location.optBoolean("inside")) { gymBoard = null; requestedGymRevision = -1 }
        packet.optJSONObject("gym_board")?.let { gymBoard = it }
        layoutPanel()
        if (opened == "gym") {
            val revision = packet.optLong("gym_revision")
            if (gymBoard?.optLong("revision") != revision && requestedGymRevision != revision) {
                requestedGymRevision = revision
                main.post { if (opened == "gym" && foreground) world.send(json("action" to "gym_view")) }
            }
            val board = gymBoard
            if (board != null && (board.optLong("revision") != mountedGymRevision || value?.has("gym_board") == true)) {
                try {
                    val boardView = gym.build(board)
                    if (panelBody.getChildAt(0) !== boardView) {
                        panelBody.removeAllViews(); (boardView.parent as? ViewGroup)?.removeView(boardView)
                        panelBody.addView(boardView, LinearLayout.LayoutParams(-1, -1))
                    }
                    mountedGymRevision = board.optLong("revision")
                }
                catch (_: Exception) {
                    panelBody.removeAllViews()
                    panelBody.addView(label("The Gym board could not be displayed.", "gym-render-error"))
                }
            }
            gym.refreshError(panelBody, board)
        }
    }

    private fun layoutPanel() {
        if (opened.isEmpty() || safe.width <= 0 || safe.height <= 0) return
        val availableWidth = (safe.width - safe.paddingLeft - safe.paddingRight).coerceAtLeast(1)
        val availableHeight = (safe.height - safe.paddingTop - safe.paddingBottom - dp(60)).coerceAtLeast(dp(80))
        val width = minOf(availableWidth, dp(540))
        val reading = opened == "computer" && reader.snapshot?.optBoolean("reading") == true && !pairing
        val height = if (reading || opened == "gym") availableHeight else
            minOf(availableHeight, maxOf(dp(340), minOf(dp(560), (availableHeight * 0.75).toInt())))
        val anchor = latestWorld?.optJSONObject(if (opened == "gym") "gym" else "computer")
        val anchorX = ((anchor?.optDouble("screen_x", 0.5) ?: 0.5) * root.width).toInt()
        val anchorY = ((anchor?.optDouble("screen_y", 0.5) ?: 0.5) * root.height).toInt()
        val left = (anchorX - safe.paddingLeft - width / 2).coerceIn(0, availableWidth - width)
        val top = if (reading || opened == "gym") dp(60) else
            (anchorY + dp(30) - safe.paddingTop).coerceIn(dp(60), dp(60) + availableHeight - height)
        val params = panel.layoutParams as FrameLayout.LayoutParams
        if (params.width != width || params.height != height || params.leftMargin != left || params.topMargin != top) {
            params.width = width; params.height = height; params.leftMargin = left; params.topMargin = top
            panel.layoutParams = params
        }
    }

    private fun placeAnchor(button: Button, location: JSONObject, allowed: Boolean) {
        button.visibility = if (allowed && location.optBoolean("visible")) View.VISIBLE else View.GONE
        button.isEnabled = foreground && location.optBoolean("near")
        val width = if (button.width > 0) button.width else dp(180)
        val x = (location.optDouble("screen_x", 0.5).coerceIn(0.0, 1.0) * root.width).toFloat() - width / 2
        val y = (location.optDouble("screen_y", 0.5).coerceIn(0.0, 1.0) * root.height).toFloat() - dp(50)
        button.translationX = x.coerceIn(safe.paddingLeft.toFloat(), maxOf(safe.paddingLeft.toFloat(), root.width - safe.paddingRight - width.toFloat()))
        button.translationY = y.coerceIn((safe.paddingTop + dp(64)).toFloat(), maxOf((safe.paddingTop + dp(64)).toFloat(), (root.height - safe.paddingBottom - dp(220)).toFloat()))
    }

    private fun renderComputer(force: Boolean = false) {
        if (opened != "computer" || !::panelBody.isInitialized) return
        val packet = reader.snapshot
        val paired = packet?.optBoolean("paired") == true
        val reading = packet?.optBoolean("reading") == true && !pairing
        val mode = if (!paired || pairing) "pair" else "chats"
        if (force || mode != computerMode || reading != lastReading) {
            computerMode = mode; lastReading = reading
            stopCamera(); renderer.clear(); panelBody.removeAllViews(); readerContent = null
            readerError = label("", "reader-error"); panelBody.addView(readerError)
            readerStatus = label("", "reader-status", 11f); panelBody.addView(readerStatus)
            if (mode == "pair") buildPairing(paired) else buildChats(reading)
        }
        val error = reader.error ?: packet?.textOrNull("error")
        readerError?.text = error.orEmpty(); readerError?.visibility = if (error == null) View.GONE else View.VISIBLE
        readerStatus?.text = if (reader.busy) "Connecting or updating…" else packet?.optString("status") ?: "Opening protected local state"
        layoutPanel()
        if (mode == "chats" && packet != null) {
            try { readerContent?.let { renderer.mount(it, packet) } }
            catch (_: Exception) { readerError?.text = "This native view could not be displayed."; readerError?.visibility = View.VISIBLE }
        }
    }

    private fun buildPairing(paired: Boolean) {
        val body = column()
        panelBody.addView(ScrollView(this).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
        body.addView(label("Connect your computer", size = 18f))
        body.addView(label("Run this in the OpenAgents repository on your computer, then scan its QR invitation."))
        val command = "cargo run --release -p coder-connect -- connect"
        body.addView(label(command, "computer-command", 12f))
        body.addView(button("Copy command", "computer-copy-command") { copy("Connect command", command) })
        body.addView(button("Scan QR code", "computer-scan") {
            scanning = true; requestingCamera = true; cameraPermission.launch(Manifest.permission.CAMERA)
        })
        scannerContainer = column(); body.addView(scannerContainer)
        val input = EditText(this).apply {
            hint = "Computer invitation"; tag = "computer-code"; minLines = 3; maxLines = 5
            isSaveEnabled = false; importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            setTextColor(AMBER); visibility = View.GONE
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE or android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            filters = arrayOf(android.text.InputFilter.LengthFilter(65_536))
        }
        val connect = button("Connect to computer", "computer-connect") { submitCode(input.text.toString()); input.text.clear() }.apply { visibility = View.GONE }
        body.addView(button("Paste code", "computer-paste") { stopCamera(); input.visibility = View.VISIBLE; connect.visibility = View.VISIBLE; input.requestFocus() })
        body.addView(input); body.addView(connect)
        body.addView(label("This connection can read saved chats. It cannot run commands or approve work.", size = 12f))
        if (paired) body.addView(button("Back to chats", "computer-chats") { pairing = false; renderComputer(true) })
        worldConnection(body)
    }

    private fun buildChats(reading: Boolean) {
        val actions = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        if (!reading) actions.addView(button("Connect computer", "computer-pair") { pairing = true; renderComputer(true) }, LinearLayout.LayoutParams(0, -2, 1f))
        actions.addView(button("Refresh", "reader-refresh") { reader.refresh(true) })
        panelBody.addView(actions)
        if (!reading) panelBody.addView(label("Read-only", "reader-read-only", 12f))
        readerContent = column(); panelBody.addView(readerContent, LinearLayout.LayoutParams(-1, 0, 1f))
        if (!reading) {
            panelBody.addView(button("Device details", "reader-details") { details = !details; renderComputer(true) })
            if (details) {
                panelBody.addView(label(reader.snapshot?.optString("public_key").orEmpty(), "reader-public-key", 11f))
                panelBody.addView(button("Disconnect this computer", "reader-disconnect") {
                    val confirm = column()
                    confirm.addView(label("Erase cached chats on this phone? The computer's files stay unchanged."))
                    confirm.addView(button("Disconnect and erase", "reader-disconnect-confirm") {
                        reader.request(json("op" to "disconnect")); details = false
                    })
                    confirm.addView(button("Keep connection", "reader-disconnect-cancel") { panelBody.removeView(confirm) })
                    panelBody.addView(confirm)
                })
            }
            worldConnection(panelBody)
        }
    }

    private fun worldConnection(body: LinearLayout) {
        body.addView(button("World connection", "world-connection") { worldDetails = !worldDetails; renderComputer(true) })
        if (!worldDetails) return
        body.addView(label("Joining publishes this device's world presence and movement with a separate Verse identity.", size = 12f))
        val relay = EditText(this).apply { hint = "wss://relay.example.com"; tag = "world-relay"; setTextColor(AMBER); inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI }
        body.addView(relay)
        body.addView(button("Join relay", "world-join") { world.send(json("action" to "connect", "relay" to relay.text.toString())) })
        body.addView(button("Leave relay", "world-leave") { world.send(json("action" to "disconnect")) })
    }
    private fun submitCode(code: String) {
        stopCamera()
        if (code.toByteArray().size > 65_536) { readerError?.text = "The connection code is too large. Copy a fresh invitation."; readerError?.visibility = View.VISIBLE; return }
        reader.request(json("op" to "connect", "code" to code)) { success ->
            if (success) { pairing = false; renderComputer(true) }
        }
    }
    private fun startCamera() {
        val box = scannerContainer ?: return
        box.removeAllViews()
        scanner.start(box) { result ->
            if (scanning && foreground) result.fold({ submitCode(it) }, { cameraFailure(it.message ?: "The camera could not read this invitation.") })
        }
        box.addView(button("Stop scanning", "computer-scan-stop") { stopCamera() })
    }
    private fun cameraFailure(message: String) {
        scanner.stop(); scannerContainer?.removeAllViews()
        scannerContainer?.addView(label(message, "camera-status"))
        scannerContainer?.addView(button("Open camera settings", "camera-settings") {
            startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:$packageName")))
        })
    }
    private fun stopCamera() { scanning = false; requestingCamera = false; if (::scanner.isInitialized) scanner.stop(); scannerContainer?.removeAllViews() }
    private fun copy(name: String, value: String) = getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(name, value))
    private fun closePanel() {
        stopCamera(); pairing = false
        world.send(json("action" to if (opened == "gym") "close_gym" else "close_computer"))
    }
    override fun onResume() {
        super.onResume(); foreground = true
        if (::world.isInitialized) world.setResumed(true)
        if (::reader.isInitialized) reader.foreground(opened == "computer")
        if (scanning && !requestingCamera && opened == "computer") startCamera()
    }
    override fun onPause() {
        foreground = false
        if (requestingCamera) { scanner.stop(); scannerContainer?.removeAllViews() } else stopCamera()
        if (::world.isInitialized) world.setResumed(false)
        if (::reader.isInitialized) reader.foreground(false)
        super.onPause()
    }
    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        if (::scanner.isInitialized) scanner.dispose()
        if (::world.isInitialized) world.release()
        if (::reader.isInitialized) reader.dispose()
        super.onDestroy()
    }
}
