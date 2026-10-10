package com.openagents.coder

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.drawable.BitmapDrawable
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
import android.widget.ImageView
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.ScrollView
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
    private lateinit var computersRenderer: NativeRenderer
    // Everglade's Agent Studio panel: a Rust Native view the world returns.
    private lateinit var studioRenderer: NativeRenderer
    private lateinit var gym: GymPanel
    private lateinit var doorError: TextView
    private lateinit var doorRetry: Button
    private lateinit var worldError: TextView
    private lateinit var retry: Button
    private lateinit var controls: LinearLayout
    private lateinit var cameraButton: ImageButton
    private lateinit var recenter: ImageButton
    private lateinit var motionStatus: TextView
    private lateinit var gymButton: Button
    private lateinit var panel: LinearLayout
    private lateinit var panelBody: LinearLayout
    private var readerContent: LinearLayout? = null
    private var readerError: TextView? = null
    private var readerStatus: TextView? = null
    private var pushStatus: TextView? = null
    private var scannerContainer: LinearLayout? = null
    // The Computers surface: its own Rust Native tree and input requests.
    private var computers = false
    private var settings = false
    private var settingsToggle: Button? = null
    private var headerTitle: TextView? = null
    private var headerRefresh: Button? = null
    private var worldConnectionStatus: TextView? = null
    private var worldConnectionError: TextView? = null
    private var computersContent: LinearLayout? = null
    private var computersInput: LinearLayout? = null
    private var computersQr: ImageView? = null
    private var shownQr: String? = null
    /** A computer's screenshot or image file (#11185), drawn from Rust's grid. */
    private var computersCapture: ImageView? = null
    private var shownCapture: String? = null
    private var computersToken: String? = null
    private var handledExit: JSONObject? = null
    private var scanned: (String) -> Unit = { submitCode(it) }
    private var computerMode = ""
    private var opened = ""
    private var pairing = false
    private var details = false
    private var worldDetails = false
    private var verseAbout = false
    private var verseCredits: String? = null
    private var scanning = false
    private var requestingCamera = false
    private var foreground = false
    private var synthetic = false
    private var loopbackTest = false
    private var latestWorld: JSONObject? = null
    private var gymBoard: JSONObject? = null
    private var requestedGymRevision = -1L
    private var mountedGymRevision = -1L
    private var studioView: JSONObject? = null
    private var requestedStudioRevision = -1L
    private var mountedStudioRevision = -1L
    private var lastReading = false
    private val main = Handler(Looper.getMainLooper())
    private val refresh = object : Runnable {
        override fun run() {
            if (foreground && opened == "computer" && !pairing && !scanning && !computers && !settings) reader.refresh()
            // Host status moves on its own; poll while the Computers screens show.
            if (foreground && opened == "computer" && computers && !scanning) reader.pollComputers()
            main.postDelayed(this, 5000)
        }
    }
    private val cameraPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { allowed ->
        requestingCamera = false
        if (scanning && opened == "computer") {
            if (allowed) { if (foreground) startCamera() } else cameraFailure("Camera access is off. Enable it in Settings, or paste the invitation.")
        }
    }

    // Push builds only: wakes still register when notifications are declined.
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { allowed ->
        if (!allowed) reader.pushFailed("Notifications are off for Coder. Turn them on in Settings to see wakes.")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        synthetic = BuildConfig.DEBUG && intent.getBooleanExtra("synthetic", false)
        // Test launches only: admit a ws:// loopback relay and a host on this machine.
        loopbackTest = BuildConfig.DEBUG && intent.getBooleanExtra("loopback_test", false)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = Color.TRANSPARENT; window.navigationBarColor = Color.TRANSPARENT
        if (android.os.Build.VERSION.SDK_INT >= 29) window.isNavigationBarContrastEnforced = false
        if (android.os.Build.VERSION.SDK_INT >= 28) window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = false; isAppearanceLightNavigationBars = false
        }
        val storage = DeviceStorage(this, synthetic, if (synthetic) intent.getStringExtra("door_scope") else null)
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
        computersRenderer = NativeRenderer(this, { view, node ->
            reader.request(json("op" to "computers_activate", "instance" to view.getString("instance"),
                "revision" to view.getLong("revision"), "node" to node))
        }, { _, _ -> }, "computers")
        studioRenderer = NativeRenderer(this, { view, node ->
            world.send(json("action" to "studio_activate", "instance" to view.getString("instance"),
                "revision" to view.getLong("revision"), "node" to node))
        }, { _, _ -> }, "studio_view")
        gym = GymPanel(this, world)
        reader = ReaderBridge(storage, synthetic, loopbackTest) { if (opened == "computer") renderComputer() }
        setContentView(root)
        if (!synthetic || loopbackTest) PushSettings.start(this, reader, notificationPermission)
        ViewCompat.setOnApplyWindowInsetsListener(safe) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            val density = resources.displayMetrics.density
            world.setHudInsets(bars.top / density, bars.right / density, bars.bottom / density, bars.left / density)
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
        worldError = label("", "verse-error").apply { visibility = View.GONE }; header.addView(worldError)
        retry = button("Retry world renderer", "verse-retry") { world.retry() }.apply { visibility = View.GONE }; header.addView(retry)
        doorError = label("", "door-storage-error").apply { visibility = View.GONE }; header.addView(doorError)
        doorRetry = button("Retry saving choices", "door-save-retry") { world.retryDoorPreferences() }.apply { visibility = View.GONE }; header.addView(doorRetry)
        safe.addView(header, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
        controls = column()
        val modes = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.END }
        cameraButton = iconButton(R.drawable.ic_touch_look, "Touch look", "verse-camera-mode") { world.toggleMotion() }
        modes.addView(cameraButton, LinearLayout.LayoutParams(dp(48), dp(48)))
        recenter = iconButton(R.drawable.ic_recenter, "Recenter", "verse-motion-recenter") { world.send(json("action" to "recenter_camera")) }
        modes.addView(recenter, LinearLayout.LayoutParams(dp(48), dp(48)))
        controls.addView(modes)
        motionStatus = label("", "verse-motion-error", 11f); controls.addView(motionStatus)
        safe.addView(controls, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END))
        gymButton = button("Gym board", "gym-interact") { world.send(json("action" to "interact_gym")) }.apply { visibility = View.GONE }
        root.addView(gymButton, FrameLayout.LayoutParams(dp(190), -2))
        panel = column().apply {
            visibility = View.GONE
            setPadding(dp(12), dp(10), dp(12), dp(10))
            background = GradientDrawable().apply { setColor(0xfa060500.toInt()); cornerRadius = dp(16).toFloat(); setStroke(dp(1), AMBER) }
            isClickable = true
        }
        safe.addView(panel, FrameLayout.LayoutParams(-1, -1))
    }

    private fun iconButton(icon: Int, description: String, key: String, action: () -> Unit) = ImageButton(this).apply {
        setImageResource(icon); setColorFilter(AMBER); tag = key; contentDescription = description
        setPadding(dp(12), dp(12), dp(12), dp(12))
        background = GradientDrawable().apply { setColor(0xbb060500.toInt()); cornerRadius = dp(24).toFloat() }
        setOnClickListener { action() }
    }

    private fun receiveWorld(value: JSONObject?, error: String?) {
        if (!::worldError.isInitialized) return
        if (value != null) latestWorld = value
        val packet = latestWorld
        val problem = error ?: packet?.textOrNull("error")
        val doorProblem = world.doorStorageError ?: packet?.optJSONObject("doors")?.textOrNull("error")
        doorError.text = doorProblem.orEmpty()
        doorError.visibility = if (doorProblem == null) View.GONE else View.VISIBLE
        doorRetry.visibility = if (world.canRetryDoorSave) View.VISIBLE else View.GONE
        worldError.text = problem.orEmpty(); worldError.visibility = if (problem == null) View.GONE else View.VISIBLE
        retry.visibility = worldError.visibility
        if (packet == null) return
        val newPanel = when {
            packet.optBoolean("computer_open") -> "computer"; packet.optBoolean("gym_open") -> "gym"
            packet.optBoolean("studio_open") -> "studio"; else -> ""
        }
        if (newPanel != opened) {
            clearComputersValue()
            opened = newPanel; computerMode = ""; computers = false; settings = false; renderer.clear(); computersRenderer.clear(); studioRenderer.clear(); stopCamera(); controls.visibility = if (opened.isEmpty()) View.VISIBLE else View.GONE
            reader.foreground(foreground && opened == "computer")
            if (opened.isEmpty()) panel.visibility = View.GONE else if (opened == "studio") {
                // The studio view carries its own title and close control.
                panel.visibility = View.VISIBLE; panel.removeAllViews()
                panelBody = column(); panel.addView(panelBody, LinearLayout.LayoutParams(-1, 0, 1f))
                mountedStudioRevision = -1
            } else {
                panel.visibility = View.VISIBLE; panel.removeAllViews()
                val header = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
                headerTitle = label(if (opened == "computer") "Chats" else "Gym", size = 18f)
                header.addView(headerTitle, LinearLayout.LayoutParams(0, -2, 1f))
                if (opened == "computer") {
                    headerRefresh = button("↻", "reader-refresh") { reader.refresh(true) }.apply {
                        contentDescription = "Refresh chats"
                        minWidth = dp(40); minimumWidth = dp(40)
                    }
                    header.addView(headerRefresh)
                    settingsToggle = button("…", "computer-settings") {
                        settings = !settings && !computers
                        computers = false; pairing = false
                        renderComputer(true)
                    }.apply { contentDescription = "Computer settings"; minWidth = dp(40); minimumWidth = dp(40) }
                    header.addView(settingsToggle)
                }
                header.addView(button("×", if (opened == "computer") "computer-close" else "gym-close") { closePanel() }.apply {
                    contentDescription = "Back to world"; minWidth = dp(40); minimumWidth = dp(40)
                })
                panel.addView(header)
                panelBody = column(); panel.addView(panelBody, LinearLayout.LayoutParams(-1, 0, 1f))
                if (opened == "computer") renderComputer(true) else mountedGymRevision = -1
            }
        }
        val location = packet.getJSONObject("gym")
        placeAnchor(gymButton, location, opened.isEmpty() && location.optBoolean("inside"))
        gymButton.text = if (location.optBoolean("near")) "Open Gym board" else "Gym board"
        val motion = packet.optString("camera_mode") == "motion"
        cameraButton.contentDescription = if (motion) "Motion look" else "Touch look"
        cameraButton.setImageResource(if (motion) R.drawable.ic_motion_look else R.drawable.ic_touch_look)
        cameraButton.isEnabled = foreground && world.motionAvailable
        recenter.isEnabled = foreground
        motionStatus.text = world.motionError ?: if (!world.motionAvailable) "Motion look is unavailable on this device." else ""
        motionStatus.visibility = if (motionStatus.text.isEmpty()) View.GONE else View.VISIBLE
        if (!packet.optBoolean("gym_active") || !location.optBoolean("inside")) { gymBoard = null; requestedGymRevision = -1 }
        packet.optJSONObject("gym_board")?.let { gymBoard = it }
        layoutPanel()
        if (opened == "computer" && settings) updateWorldConnection()
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
        if (opened != "studio") { studioView = null; requestedStudioRevision = -1 }
        packet.optJSONObject("studio_view")?.let { studioView = it }
        if (opened == "studio") {
            // Rust rebuilds the view as the studio changes; ask by revision.
            val revision = packet.optLong("studio_revision")
            if (studioView?.optLong("revision") != revision && requestedStudioRevision != revision) {
                requestedStudioRevision = revision
                main.post { if (opened == "studio" && foreground) world.send(json("action" to "studio_view")) }
            }
            val view = studioView
            if (view != null && view.optLong("revision") != mountedStudioRevision) {
                try {
                    studioRenderer.mount(panelBody, json("studio_view" to view))
                    mountedStudioRevision = view.optLong("revision")
                } catch (_: Exception) {
                    panelBody.removeAllViews(); studioRenderer.clear()
                    panelBody.addView(label("The studio panel could not be displayed.", "studio-render-error"))
                }
            }
        }
    }

    private fun layoutPanel() {
        if (opened.isEmpty() || safe.width <= 0 || safe.height <= 0) return
        val availableWidth = (safe.width - safe.paddingLeft - safe.paddingRight).coerceAtLeast(1)
        val availableHeight = (safe.height - safe.paddingTop - safe.paddingBottom).coerceAtLeast(dp(80))
        val width = minOf(availableWidth, dp(540))
        val height = availableHeight
        // The studio panel has no anchor in view; it centers.
        val anchor = if (opened == "studio") null else latestWorld?.optJSONObject(if (opened == "gym") "gym" else "computer")
        val anchorX = ((anchor?.optDouble("screen_x", 0.5) ?: 0.5) * root.width).toInt()
        val left = (anchorX - safe.paddingLeft - width / 2).coerceIn(0, availableWidth - width)
        val top = 0
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
        if (computers && packet != null && packet !== handledExit && packet.optBoolean("computers_exit")) {
            // First run finished: return to the existing pairing and chats flow.
            handledExit = packet; computers = false; settings = false
        }
        val mode = if (computers) "computers" else if (settings) "settings" else if (!paired || pairing) "pair" else "chats"
        headerTitle?.text = when (mode) { "settings" -> "Settings"; "computers" -> "Computers"; "pair" -> "Pair computer"; else -> "Chats" }
        settingsToggle?.text = if (settings || computers) "Chats" else "…"
        settingsToggle?.contentDescription = if (settings || computers) "Back to chats" else "Computer settings"
        headerRefresh?.visibility = if (mode == "chats") View.VISIBLE else View.GONE
        headerRefresh?.isEnabled = !reader.busy
        if (force || mode != computerMode || reading != lastReading) {
            computerMode = mode; lastReading = reading
            clearComputersValue()
            stopCamera(); renderer.clear(); computersRenderer.clear(); panelBody.removeAllViews(); readerContent = null
            computersContent = null; computersInput = null; computersToken = null; computersQr = null; shownQr = null; computersCapture = null; shownCapture = null
            worldConnectionStatus = null; worldConnectionError = null
            readerError = label("", "reader-error"); panelBody.addView(readerError)
            readerStatus = label("", "reader-status", 11f); panelBody.addView(readerStatus)
            pushStatus = null
            when (mode) { "computers" -> buildComputers(); "settings" -> buildSettings(paired); "pair" -> buildPairing(paired); else -> buildChats() }
        }
        val error = reader.error ?: packet?.textOrNull("error")
        readerError?.text = error.orEmpty(); readerError?.visibility = if (error == null) View.GONE else View.VISIBLE
        readerStatus?.text = "Updating…"
        readerStatus?.visibility = if (reader.busy) View.VISIBLE else View.GONE
        // Only builds configured for push have a wake status, in Device details.
        val wakes = reader.pushStatus
        pushStatus?.text = wakes.orEmpty(); pushStatus?.visibility = if (wakes == null) View.GONE else View.VISIBLE
        layoutPanel()
        if (mode == "chats" && packet != null) {
            try { readerContent?.let { renderer.mount(it, packet) } }
            catch (_: Exception) { readerError?.text = "This native view could not be displayed."; readerError?.visibility = View.VISIBLE }
        }
        if (mode == "computers" && packet != null) {
            try { computersContent?.let { computersRenderer.mount(it, packet) } }
            catch (_: Exception) { readerError?.text = "This native view could not be displayed."; readerError?.visibility = View.VISIBLE }
            renderComputersQr(packet.optJSONObject("computers_qr"))
            renderComputersCapture(packet.optJSONObject("computers_capture"))
            renderComputersInput(packet.optJSONObject("computers_input"))
        }
    }

    /** Draw the invitation QR modules Rust rendered on this device, one pixel per module, unfiltered. */
    private fun renderComputersQr(qr: JSONObject?) {
        val view = computersQr ?: return
        val rows = qr?.optJSONArray("rows")
        val key = rows?.toString()
        if (key == shownQr) return
        shownQr = key
        if (rows == null || rows.length() == 0 || rows.length() > 256) { view.setImageDrawable(null); view.visibility = View.GONE; return }
        val size = rows.length()
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        for (y in 0 until size) {
            val row = rows.optString(y)
            for (x in 0 until size) bitmap.setPixel(x, y, if (row.getOrNull(x) == '1') Color.BLACK else Color.WHITE)
        }
        view.setImageDrawable(BitmapDrawable(resources, bitmap).apply { isFilterBitmap = false })
        view.visibility = View.VISIBLE
    }

    /**
     * Draw a computer's picture from the grid Rust decoded on this device
     * (#11185): one pixel per cell, `0` darkest to `9` brightest, in amber.
     */
    private fun renderComputersCapture(capture: JSONObject?) {
        val view = computersCapture ?: return
        val rows = capture?.optJSONArray("rows")
        val key = capture?.optString("resource")
        if (key == shownCapture) return
        shownCapture = key
        val width = capture?.optInt("width") ?: 0
        if (rows == null || rows.length() == 0 || rows.length() > 96 || width <= 0 || width > 128) {
            view.setImageDrawable(null); view.visibility = View.GONE; return
        }
        val bitmap = Bitmap.createBitmap(width, rows.length(), Bitmap.Config.ARGB_8888)
        for (y in 0 until rows.length()) {
            val row = rows.optString(y)
            for (x in 0 until width) {
                val level = ((row.getOrNull(x) ?: '0') - '0').coerceIn(0, 9)
                bitmap.setPixel(x, y, Color.rgb(255 * level / 9, 176 * level / 9, 0))
            }
        }
        view.setImageDrawable(BitmapDrawable(resources, bitmap).apply { isFilterBitmap = false })
        view.contentDescription = capture.optString("label")
        view.visibility = View.VISIBLE
    }

    private fun buildComputers() {
        val body = column()
        panelBody.addView(ScrollView(this).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
        computersContent = column(); body.addView(computersContent)
        computersQr = ImageView(this).apply {
            tag = "computers-qr"; contentDescription = "Invitation QR code"; visibility = View.GONE
            scaleType = ImageView.ScaleType.FIT_CENTER
        }
        body.addView(computersQr, LinearLayout.LayoutParams(dp(240), dp(240)))
        computersCapture = ImageView(this).apply {
            tag = "computers-capture"; visibility = View.GONE
            scaleType = ImageView.ScaleType.FIT_CENTER; adjustViewBounds = true
        }
        body.addView(computersCapture, LinearLayout.LayoutParams(-1, -2))
        computersInput = column(); body.addView(computersInput)
    }

    /** Show the one native field or scanner Rust asked for. Rust validates the value. */
    private fun renderComputersInput(input: JSONObject?) {
        val box = computersInput ?: return
        val token = input?.optString("token")
        if (token == computersToken) return
        clearComputersValue()
        computersToken = token; stopCamera(); box.removeAllViews()
        if (input == null || token == null) return
        box.addView(label(input.optString("prompt"), size = 12f))
        val limit = input.optInt("max_bytes", 16_384)
        if (input.optBoolean("scan")) {
            box.addView(button("Scan QR code", "computers-scan") {
                scanned = { submitComputers(token, it, limit) }
                scanning = true; requestingCamera = true; cameraPermission.launch(Manifest.permission.CAMERA)
            })
            scannerContainer = column(); box.addView(scannerContainer)
        }
        // A secret is masked, single-line, and never saved, suggested, or autofilled.
        val secret = input.optBoolean("secret", false)
        val field = EditText(this).apply {
            hint = input.optString("label"); contentDescription = input.optString("label"); tag = "computers-input"
            minLines = 1; maxLines = if (secret) 1 else 4; isSaveEnabled = false; setTextColor(AMBER)
            importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
                if (secret) android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD else android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE
            if (secret) {
                transformationMethod = android.text.method.PasswordTransformationMethod.getInstance()
                imeOptions = imeOptions or android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
            }
            filters = arrayOf(android.text.InputFilter.LengthFilter(limit))
        }
        box.addView(field)
        box.addView(button("Submit", "computers-submit") { submitComputers(token, field.text.toString(), limit) })
        box.addView(button("Cancel", "computers-cancel") {
            clearComputersValue(); stopCamera()
            reader.request(json("op" to "computers_cancel", "token" to token))
        })
    }

    private fun clearComputersValue() {
        computersInput?.findViewWithTag<EditText>("computers-input")?.text?.clear()
    }

    private fun submitComputers(token: String, value: String, limit: Int) {
        clearComputersValue(); stopCamera()
        if (value.toByteArray().size > limit) { readerError?.text = "That's too long. Copy it again."; readerError?.visibility = View.VISIBLE; return }
        reader.request(json("op" to "computers_input", "token" to token, "value" to value))
    }

    private fun buildPairing(paired: Boolean) {
        val body = column()
        panelBody.addView(ScrollView(this).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
        body.addView(label("In a terminal on the computer:"))
        val command = "openagents pair"
        body.addView(label(command, "computer-command", 12f))
        body.addView(button("Copy command", "computer-copy-command") { copy("Connect command", command) })
        body.addView(button("Scan QR code", "computer-scan") {
            scanned = { submitCode(it) }
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
        if (paired) body.addView(button("Back to chats", "computer-chats") { pairing = false; renderComputer(true) })
    }

    private fun buildChats() {
        readerContent = column(); panelBody.addView(readerContent, LinearLayout.LayoutParams(-1, 0, 1f))
    }

    private fun buildSettings(paired: Boolean) {
        val body = column()
        panelBody.addView(ScrollView(this).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
        body.addView(button("Pair computer", "computer-pair") {
            settings = false; pairing = true; renderComputer(true)
        })
        body.addView(button("Computers", "computers-toggle") {
            settings = false; computers = true
            renderComputer(true)
            reader.request(json("op" to "computers_refresh"))
        })
        worldConnection(body)
        body.addView(button("About Verse", "verse-about") {
            verseAbout = !verseAbout
            if (verseAbout && verseCredits == null) {
                verseCredits = world.send(json("action" to "zone_credits"))?.textOrNull("credits")
            }
            renderComputer(true)
        })
        if (verseAbout) body.addView(label(verseCredits ?: "Notices are unavailable.", "verse-credits", 11f).apply { setTextIsSelectable(true) })
        body.addView(button("Device details", "reader-details") { details = !details; renderComputer(true) })
        if (details) {
            body.addView(label(reader.snapshot?.optString("public_key").orEmpty(), "reader-public-key", 11f))
            body.addView(label(reader.snapshot?.optString("status").orEmpty(), "reader-connection-status", 11f))
            pushStatus = label("", "push-status", 11f).also { body.addView(it) }
            if (paired) body.addView(button("Disconnect computer", "reader-disconnect") {
                val confirm = column()
                confirm.addView(label("Erase cached chats on this phone?"))
                confirm.addView(button("Disconnect and erase", "reader-disconnect-confirm") {
                    reader.request(json("op" to "disconnect")); details = false
                })
                confirm.addView(button("Cancel", "reader-disconnect-cancel") { body.removeView(confirm) })
                body.addView(confirm)
            })
        }
    }

    private fun worldConnection(body: LinearLayout) {
        body.addView(button("World connection", "world-connection") { worldDetails = !worldDetails; renderComputer(true) })
        if (!worldDetails) return
        worldConnectionStatus = label("", "world-connection-status", 12f).also { body.addView(it) }
        worldConnectionError = label("", "world-connection-error", 12f).also { body.addView(it) }
        val relay = EditText(this).apply {
            hint = "World relay"; contentDescription = "World relay URL"; tag = "world-relay"
            setTextColor(AMBER)
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI
            setText(latestWorld?.optJSONObject("connection")?.textOrNull("relay") ?: "wss://relay.openagents.com")
        }
        body.addView(relay)
        val actions = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        actions.addView(button("Join", "world-join") {
            world.send(json("action" to "connect", "relay" to relay.text.toString().trim()))
        }, LinearLayout.LayoutParams(0, -2, 1f))
        actions.addView(button("Leave", "world-leave") { world.send(json("action" to "disconnect")) }, LinearLayout.LayoutParams(0, -2, 1f))
        body.addView(actions)
        updateWorldConnection()
    }

    private fun updateWorldConnection() {
        val connection = latestWorld?.optJSONObject("connection")
        worldConnectionStatus?.text = connection?.optString("label") ?: "Offline"
        val error = world.worldStorageError ?: connection?.textOrNull("error") ?: latestWorld?.textOrNull("error")
        worldConnectionError?.text = error.orEmpty()
        worldConnectionError?.visibility = if (error == null) View.GONE else View.VISIBLE
    }
    private fun submitCode(code: String) {
        stopCamera()
        if (code.toByteArray().size > 65_536) { readerError?.text = "The connection code is too large. Copy a fresh invitation."; readerError?.visibility = View.VISIBLE; return }
        reader.request(json("op" to "connect", "code" to code)) { success ->
            if (success) { pairing = false; settings = false; renderComputer(true) }
        }
    }
    private fun startCamera() {
        val box = scannerContainer ?: return
        box.removeAllViews()
        scanner.start(box) { result ->
            if (scanning && foreground) result.fold({ scanned(it) }, { cameraFailure(it.message ?: "The camera could not read this invitation.") })
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
        clearComputersValue()
        stopCamera(); pairing = false; computers = false; settings = false
        world.send(json("action" to when (opened) { "gym" -> "close_gym"; "studio" -> "close_studio"; else -> "close_computer" }))
    }
    override fun onResume() {
        super.onResume(); foreground = true
        if (::world.isInitialized) world.setResumed(true)
        if (::reader.isInitialized) reader.foreground(opened == "computer")
        if (::reader.isInitialized) reader.lifecycle(true)
        if (scanning && !requestingCamera && opened == "computer") startCamera()
    }
    override fun onPause() {
        clearComputersValue()
        foreground = false
        if (requestingCamera) { scanner.stop(); scannerContainer?.removeAllViews() } else stopCamera()
        if (::world.isInitialized) world.setResumed(false)
        if (::reader.isInitialized) reader.foreground(false)
        if (::reader.isInitialized) reader.lifecycle(false)
        super.onPause()
    }
    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        if (::scanner.isInitialized) scanner.dispose()
        if (::world.isInitialized) world.release()
        if (::reader.isInitialized) reader.dispose()
        PushPlatform.stop()
        super.onDestroy()
    }
}
