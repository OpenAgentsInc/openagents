// Rust builds every screen; this host renders them and supplies the pieces a
// Rust Native tree cannot: tabs, navigation, the camera, keyboards, the
// terminal's keyboard target, and the Verse surface.
package com.openagents.app

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.Typeface
import android.graphics.drawable.BitmapDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONObject

/** The four tabs, shown as icons; each keeps a spoken name for TalkBack. */
enum class AppTab(val title: String, val icon: Int) {
    CODER("Coder", R.drawable.ic_tab_coder),
    VERSE("Verse", R.drawable.ic_tab_verse),
    WALLET("Wallet", R.drawable.ic_tab_wallet),
    ACCOUNT("Account", R.drawable.ic_tab_account),
}

/** A screen that the Account tab opens. */
enum class AccountRoute(val title: String) {
    COMPUTERS("Computers"), CHATS("Chats on your computers"), TAILNET("Tailnet"), DEVICE("About this device"),
}

class MainActivity : ComponentActivity() {
    private lateinit var bridge: MobileBridge
    private lateinit var scanner: QRScanner
    private lateinit var world: VerseSurface
    private lateinit var terminal: TerminalScreen
    private val main = Handler(Looper.getMainLooper())
    private var tab = AppTab.CODER
    private var route: AccountRoute? = null
    private var resumed = false
    private var ticks = 0
    private var statusTop = 0

    private val pages = mutableMapOf<AppTab, FrameLayout>()
    private val tabButtons = mutableMapOf<AppTab, ImageButton>()
    private lateinit var tabBar: LinearLayout

    // Coder tab.
    private lateinit var coderRenderer: NativeRenderer
    private val coderContent by lazy { FrameLayout(this) }

    // Account tab and its screens.
    private val accountPage by lazy { FrameLayout(this) }
    private lateinit var computersRenderer: NativeRenderer
    private lateinit var chatsRenderer: NativeRenderer
    private lateinit var tailnetRenderer: NativeRenderer
    private var routeContent: FrameLayout? = null
    private var routeNotices: LinearLayout? = null
    private var routeQr: ImageView? = null
    private var shownQr: String? = null
    private var routeProgress: ProgressBar? = null
    private var routeInput: InputBar? = null
    private var deviceKey: TextView? = null

    // Verse tab controls.
    private lateinit var cameraButton: ImageButton
    private lateinit var recenterButton: ImageButton
    private lateinit var worldStatus: TextView
    private lateinit var worldRetry: TextView

    private var cameraCallback: ((Boolean) -> Unit)? = null
    private val cameraPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        cameraCallback?.invoke(granted); cameraCallback = null
    }

    /** Asks for camera access only when a scan starts. */
    fun withCamera(callback: (Boolean) -> Unit) {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            callback(true); return
        }
        cameraCallback = callback
        cameraPermission.launch(Manifest.permission.CAMERA)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowCompat.getInsetsController(window, window.decorView).isAppearanceLightStatusBars = false
        scanner = QRScanner(this)
        bridge = MobileBridge(applicationContext) { render() }
        coderRenderer = NativeRenderer(this, { view, node -> bridge.activate("coder", view, node) },
            { token, value -> bridge.submit("coder", token, value) })
        computersRenderer = NativeRenderer(this, { view, node -> bridge.activate("computers", view, node) }, scrolling = true)
        chatsRenderer = NativeRenderer(this, { view, node -> bridge.activate("chats", view, node) })
        tailnetRenderer = NativeRenderer(this, { view, node -> bridge.activate("tailnet", view, node) })
        terminal = TerminalScreen(this, bridge)

        val root = FrameLayout(this).apply { setBackgroundColor(Palette.BACKGROUND) }
        val body = column()
        val pageHost = FrameLayout(this)
        body.addView(pageHost, LinearLayout.LayoutParams(-1, 0, 1f))
        tabBar = row().apply {
            setBackgroundColor(Palette.BACKGROUND)
            gravity = Gravity.CENTER_VERTICAL
        }
        body.addView(View(this).apply { setBackgroundColor(Palette.BORDER) }, LinearLayout.LayoutParams(-1, 1))
        body.addView(tabBar, LinearLayout.LayoutParams(-1, dp(56)))
        for (value in AppTab.entries) {
            val button = ImageButton(this).apply {
                setImageResource(value.icon)
                background = null
                contentDescription = value.title
                tag = "tab-${value.name.lowercase()}"
                setOnClickListener { select(value) }
            }
            tabButtons[value] = button
            tabBar.addView(button, LinearLayout.LayoutParams(0, -1, 1f))
            val page = FrameLayout(this).apply { visibility = View.GONE }
            pages[value] = page
            pageHost.addView(page, FrameLayout.LayoutParams(-1, -1))
        }
        pages.getValue(AppTab.CODER).addView(coderContent, FrameLayout.LayoutParams(-1, -1))
        buildVerse(pages.getValue(AppTab.VERSE))
        buildWallet(pages.getValue(AppTab.WALLET))
        pages.getValue(AppTab.ACCOUNT).addView(accountPage, FrameLayout.LayoutParams(-1, -1))
        root.addView(body, FrameLayout.LayoutParams(-1, -1))
        root.addView(terminal.root, FrameLayout.LayoutParams(-1, -1))
        setContentView(root)

        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            val typing = insets.isVisible(WindowInsetsCompat.Type.ime())
            statusTop = bars.top
            body.setPadding(bars.left, 0, bars.right, maxOf(bars.bottom, keyboard.bottom))
            terminal.root.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            tabBar.visibility = if (typing) View.GONE else View.VISIBLE
            // Every page but Verse starts below the status bar; the world
            // fills the screen behind it and keeps its controls below.
            for ((value, page) in pages) if (value != AppTab.VERSE) page.setPadding(0, bars.top, 0, 0)
            val density = resources.displayMetrics.density
            world.setHudInsets(bars.top / density, 0f, 0f, 0f)
            WindowInsetsCompat.CONSUMED
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                when {
                    tab == AppTab.ACCOUNT && route != null -> open(null)
                    tab != AppTab.CODER -> select(AppTab.CODER)
                    else -> { isEnabled = false; onBackPressedDispatcher.onBackPressed(); isEnabled = true }
                }
            }
        })

        // Developer launch extras open a tab or an Account screen directly,
        // for example `--es tab account --es account_route tailnet`.
        if (BuildConfig.DEBUG) {
            intent.getStringExtra("tab")?.let { name -> AppTab.entries.firstOrNull { it.name.equals(name, true) } }?.let { tab = it }
            intent.getStringExtra("account_route")?.let { name -> AccountRoute.entries.firstOrNull { it.name.equals(name, true) } }
                ?.let { tab = AppTab.ACCOUNT; route = it }
        }
        if (BuildConfig.DEBUG && intent.getBooleanExtra("rust_native_fixture", false)) {
            fixture = runCatching { JSONObject(assets.open("conversation.json").bufferedReader().readText()) }.getOrNull()
        }
        select(tab)
        open(route)
    }

    /** Debug builds only: Rust Native's sample conversation in place of the Coder surface. */
    private var fixture: JSONObject? = null

    // Tabs

    private fun select(value: AppTab) {
        tab = value
        for ((key, page) in pages) page.visibility = if (key == value) View.VISIBLE else View.GONE
        for ((key, button) in tabButtons) {
            button.alpha = if (key == value) 1f else 0.45f
            button.isSelected = key == value
        }
        world.setShown(value == AppTab.VERSE)
        if (value == AppTab.CODER && !bridge.busy) bridge.refreshComputers()
        render()
    }

    private fun buildWallet(page: FrameLayout) {
        page.addView(column().apply {
            gravity = Gravity.CENTER
            addView(text("Wallet", 30f).bold(), LinearLayout.LayoutParams(-2, -2).apply { gravity = Gravity.CENTER_HORIZONTAL })
            addView(text("Coming soon.", 16f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply {
                gravity = Gravity.CENTER_HORIZONTAL; topMargin = dp(8) })
        }, FrameLayout.LayoutParams(-1, -1))
    }

    private fun buildVerse(page: FrameLayout) {
        world = VerseSurface(this) { _, _ -> if (tab == AppTab.VERSE) renderVerse() }
        page.addView(world, FrameLayout.LayoutParams(-1, -1))
        // Bottom center, between the movement and look sticks Rust draws.
        val controls = column().apply { gravity = Gravity.CENTER_HORIZONTAL }
        cameraButton = iconButton(R.drawable.ic_touch_look, "Touch look", "verse-camera-mode") { world.toggleMotion() }
        recenterButton = iconButton(R.drawable.ic_recenter, "Recenter", "verse-motion-recenter") { world.recenter() }
        val modes = row().apply {
            addView(cameraButton, LinearLayout.LayoutParams(dp(48), dp(48)))
            addView(recenterButton, LinearLayout.LayoutParams(dp(48), dp(48)).apply { marginStart = dp(8) })
        }
        worldStatus = text("", 12f, Palette.SECONDARY).apply { visibility = View.GONE; tag = "verse-status" }
        worldRetry = text("Retry", 14f).apply {
            visibility = View.GONE; background = rounded(Palette.RAISED, 10f)
            setPadding(dp(14), dp(8), dp(14), dp(8)); tag = "verse-retry"
            setOnClickListener { world.retry() }
        }
        controls.addView(worldStatus, LinearLayout.LayoutParams(-2, -2).apply { bottomMargin = dp(8) })
        controls.addView(worldRetry, LinearLayout.LayoutParams(-2, -2).apply { bottomMargin = dp(8) })
        controls.addView(modes)
        page.addView(controls, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL).apply {
            setMargins(dp(16), dp(16), dp(16), dp(16)) })
    }

    private fun renderVerse() {
        val motion = world.snapshot?.optString("camera_mode") == "motion"
        cameraButton.contentDescription = if (motion) "Motion look" else "Touch look"
        cameraButton.setImageResource(if (motion) R.drawable.ic_motion_look else R.drawable.ic_touch_look)
        cameraButton.isEnabled = world.motionAvailable
        cameraButton.alpha = if (world.motionAvailable) 1f else 0.4f
        val problem = world.nativeError ?: world.snapshot?.textOrNull("error") ?: world.motionError
        worldStatus.text = problem ?: ""
        worldStatus.visibility = if (problem == null) View.GONE else View.VISIBLE
        worldRetry.visibility = if (world.nativeError != null) View.VISIBLE else View.GONE
    }

    // Account

    private fun open(next: AccountRoute?) {
        if (next != route) routeInput?.dispose()
        route = next
        accountPage.removeAllViews()
        routeContent = null; routeNotices = null; routeQr = null; shownQr = null
        routeProgress = null; routeInput = null; deviceKey = null
        computersRenderer.clear(); chatsRenderer.clear(); tailnetRenderer.clear()
        if (next == null) { accountPage.addView(accountList(), FrameLayout.LayoutParams(-1, -1)); render(); return }
        val screen = column()
        val header = row().apply {
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = dp(48)
            addView(text("‹ Account", 17f).apply {
                setPadding(dp(12), dp(10), dp(16), dp(10)); contentDescription = "Back to Account"; tag = "account-back"
                setOnClickListener { open(null) }
            })
            // The Computers, Chats, and Tailnet screens draw their own headings.
            if (next == AccountRoute.DEVICE) addView(text(next.title, 17f).bold(), LinearLayout.LayoutParams(0, -2, 1f))
        }
        screen.addView(header)
        val body = FrameLayout(this)
        screen.addView(body, LinearLayout.LayoutParams(-1, 0, 1f))
        when (next) {
            AccountRoute.COMPUTERS -> {
                val column = column()
                routeNotices = column().also { column.addView(it) }
                val content = FrameLayout(this)
                routeContent = content
                column.addView(ScrollView(this).apply { addView(content) }, LinearLayout.LayoutParams(-1, 0, 1f))
                routeQr = ImageView(this).apply {
                    visibility = View.GONE; contentDescription = "Invitation QR code"; tag = "computers-qr"
                    scaleType = ImageView.ScaleType.FIT_CENTER
                }
                column.addView(routeQr, LinearLayout.LayoutParams(dp(220), dp(220)).apply {
                    gravity = Gravity.CENTER_HORIZONTAL; setMargins(0, dp(12), 0, dp(12)) })
                routeInput = InputBar(this, scanner).also { input ->
                    column.addView(input.root, LinearLayout.LayoutParams(-1, -2).apply { setMargins(dp(8), dp(8), dp(8), dp(8)) })
                }
                body.addView(column, FrameLayout.LayoutParams(-1, -1))
            }
            AccountRoute.CHATS, AccountRoute.TAILNET -> {
                val column = column()
                routeNotices = column().also { column.addView(it) }
                val content = FrameLayout(this)
                routeContent = content
                val stack = FrameLayout(this)
                stack.addView(content, FrameLayout.LayoutParams(-1, -1))
                routeProgress = ProgressBar(this).apply {
                    visibility = View.GONE
                    contentDescription = if (next == AccountRoute.CHATS) "Loading chats" else "Checking your tailnet"
                }
                stack.addView(routeProgress, FrameLayout.LayoutParams(dp(28), dp(28), Gravity.TOP or Gravity.END).apply {
                    setMargins(dp(16), dp(16), dp(16), dp(16)) })
                column.addView(stack, LinearLayout.LayoutParams(-1, 0, 1f))
                if (next == AccountRoute.CHATS) routeInput = InputBar(this, scanner).also { input ->
                    column.addView(input.root, LinearLayout.LayoutParams(-1, -2).apply { setMargins(dp(8), dp(8), dp(8), dp(8)) })
                }
                body.addView(column, FrameLayout.LayoutParams(-1, -1))
            }
            AccountRoute.DEVICE -> body.addView(ScrollView(this).apply { addView(aboutDevice()) }, FrameLayout.LayoutParams(-1, -1))
        }
        accountPage.addView(screen, FrameLayout.LayoutParams(-1, -1))
        if (next == AccountRoute.COMPUTERS && !bridge.busy) bridge.refreshComputers()
        if (next == AccountRoute.CHATS || next == AccountRoute.TAILNET) bridge.snapshot()
        render()
    }

    private fun accountList(): View = ScrollView(this).apply {
        addView(column().apply {
            setPadding(dp(16), dp(12), dp(16), dp(24))
            addView(text("Account", 32f).bold(), LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(16) })
            addView(group(listOf(AccountRoute.COMPUTERS, AccountRoute.CHATS, AccountRoute.TAILNET)))
            addView(group(listOf(AccountRoute.DEVICE)), LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
        })
    }

    private fun group(routes: List<AccountRoute>): View = column().apply {
        background = rounded(Palette.SURFACE, 12f)
        routes.forEachIndexed { index, value ->
            if (index > 0) addView(View(this@MainActivity).apply { setBackgroundColor(Palette.BORDER) },
                LinearLayout.LayoutParams(-1, 1).apply { marginStart = dp(16) })
            addView(row().apply {
                gravity = Gravity.CENTER_VERTICAL
                minimumHeight = dp(52)
                setPadding(dp(16), 0, dp(16), 0)
                tag = "account-${value.name.lowercase()}"
                addView(text(value.title, 17f), LinearLayout.LayoutParams(0, -2, 1f))
                addView(text("›", 22f, Palette.TERTIARY))
                setOnClickListener { open(value) }
            }, LinearLayout.LayoutParams(-1, -2))
        }
    }

    private fun aboutDevice(): View = column().apply {
        setPadding(dp(16), dp(12), dp(16), dp(24))
        addView(text("DEVICE PUBLIC KEY", 13f, Palette.SECONDARY))
        deviceKey = text("Not available yet.", 14f).apply {
            typeface = Typeface.MONOSPACE; setTextIsSelectable(true)
            background = rounded(Palette.SURFACE, 12f); setPadding(dp(16), dp(14), dp(16), dp(14))
            tag = "device-key"
        }
        addView(deviceKey, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(6) })
        addView(text("APP VERSION", 13f, Palette.SECONDARY), LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
        addView(text("${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})", 16f).apply {
            setTextIsSelectable(true)
            background = rounded(Palette.SURFACE, 12f); setPadding(dp(16), dp(14), dp(16), dp(14))
        }, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(6) })
    }

    // Rendering

    private fun render() {
        if (!::bridge.isInitialized) return
        val packet = bridge.packet
        mount(coderRenderer, coderContent, fixture ?: packet?.objectOrNull("coder"))
        when (route) {
            AccountRoute.COMPUTERS -> {
                routeContent?.let { mount(computersRenderer, it, packet?.objectOrNull("computers")) }
                notices(packet, true)
                val qr = packet?.objectOrNull("computers_qr")
                routeQr?.let { image ->
                    val encoded = qr?.toString()
                    if (encoded != shownQr) {
                        shownQr = encoded
                        image.setImageDrawable(qr?.let { qrBitmap(it) }?.let { bitmap ->
                            BitmapDrawable(resources, bitmap).apply { isFilterBitmap = false } })
                    }
                    image.visibility = if (qr == null) View.GONE else View.VISIBLE
                }
                val input = packet?.objectOrNull("computers_input")
                routeInput?.show(input, bridge.busy, { bridge.submit("computers", input!!.getString("token"), it) },
                    { bridge.cancel("computers", input!!.getString("token")) })
            }
            AccountRoute.CHATS -> {
                routeContent?.let { mount(chatsRenderer, it, packet?.objectOrNull("chats")) }
                notices(packet, false)
                routeProgress?.visibility = if (packet?.optBoolean("chats_loading") == true) View.VISIBLE else View.GONE
                val input = packet?.objectOrNull("chats_input")
                routeInput?.show(input, bridge.busy, { bridge.submit("chats", input!!.getString("token"), it) },
                    { bridge.cancel("chats", input!!.getString("token")) })
            }
            AccountRoute.TAILNET -> {
                routeContent?.let { mount(tailnetRenderer, it, packet?.objectOrNull("tailnet")) }
                notices(packet, false)
                routeProgress?.visibility = if (packet?.optBoolean("tailnet_loading") == true) View.VISIBLE else View.GONE
            }
            AccountRoute.DEVICE -> deviceKey?.text = packet?.textOrNull("device") ?: "Not available yet."
            null -> Unit
        }
        terminal.update(packet?.optBoolean("terminal") == true, bridge.terminalView)
        if (tab == AppTab.VERSE) renderVerse()
    }

    private fun notices(packet: JSONObject?, app: Boolean) {
        val notices = routeNotices ?: return
        notices.removeAllViews()
        val lines = buildList {
            if (app) packet?.optJSONArray("notices")?.let { for (i in 0 until it.length()) add(it.getString(i)) }
            bridge.failure?.let { add(it) }
        }
        for (line in lines) notices.addView(text(line, 13f, Palette.SECONDARY).apply { setPadding(dp(16), dp(4), dp(16), dp(4)) })
    }

    private fun mount(renderer: NativeRenderer, container: FrameLayout, view: JSONObject?) {
        try { renderer.mount(container, view) } catch (problem: Exception) {
            renderer.clear(); container.removeAllViews()
            container.addView(text(problem.message ?: "This screen couldn't be shown.", 14f, Palette.SECONDARY).apply {
                setPadding(dp(16), dp(16), dp(16), dp(16)) })
        }
    }

    // Polling: host status and a running chat move on their own.

    private val tick = object : Runnable {
        override fun run() {
            if (!resumed) return
            ticks += 1
            val packet = bridge.packet
            val computersPolling = tab == AppTab.CODER ||
                (tab == AppTab.ACCOUNT && route == AccountRoute.COMPUTERS && packet?.objectOrNull("computers_input") == null)
            if (ticks % 3 == 0 && computersPolling && !bridge.busy) bridge.refreshComputers()
            val loading = tab == AppTab.ACCOUNT && (
                (route == AccountRoute.CHATS && packet?.optBoolean("chats_loading") == true) ||
                (route == AccountRoute.TAILNET && packet?.optBoolean("tailnet_loading") == true))
            if (loading && bridge.pending < 2) bridge.snapshot()
            main.postDelayed(this, 1000)
        }
    }

    override fun onResume() {
        super.onResume()
        resumed = true
        bridge.lifecycle(true)
        world.setResumed(true)
        main.removeCallbacks(tick)
        main.postDelayed(tick, 1000)
    }

    override fun onPause() {
        resumed = false
        main.removeCallbacks(tick)
        world.setResumed(false)
        bridge.lifecycle(false)
        super.onPause()
    }

    override fun onDestroy() {
        main.removeCallbacks(tick)
        terminal.stop()
        routeInput?.dispose()
        scanner.dispose()
        world.release()
        bridge.dispose()
        super.onDestroy()
    }

    private fun iconButton(icon: Int, label: String, key: String, action: () -> Unit) = ImageButton(this).apply {
        setImageResource(icon)
        contentDescription = label; tag = key
        background = rounded(0x99000000.toInt(), 24f, Palette.BORDER)
        setOnClickListener { action() }
    }
}
