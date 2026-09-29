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

/**
 * The four tabs, shown as icons; each keeps a spoken name for TalkBack.
 */
enum class AppTab(val title: String, val icon: Int) {
    CODER("Chat", R.drawable.ic_tab_chat),
    VERSE("Verse", R.drawable.ic_tab_verse),
    WALLET("Wallet", R.drawable.ic_tab_wallet),
    ACCOUNT("Account", R.drawable.ic_tab_account),
}

/** A screen that the Account tab opens. */
enum class AccountRoute(val title: String) {
    TRAINER("Trainer"), COMPUTERS("Computers"), TAILNET("Tailnet"), IDENTITY("Identity keys"), DEVICE("About this device"), CHANGELOG("Changelog"),
    PLAYTEST("Playtest"), REPORTS("My reports"),
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
    /** The Coder tab's requests to open Account > Computers, as last handled. */
    private var computersShown = 0
    private var statusTop = 0

    private val pages = mutableMapOf<AppTab, FrameLayout>()
    private val tabButtons = mutableMapOf<AppTab, ImageButton>()
    private lateinit var tabBar: LinearLayout

    private lateinit var wallet: WalletScreen
    private lateinit var payments: AgentPayments

    // Coder tab.
    private lateinit var coderRenderer: NativeRenderer
    private val coderContent by lazy { FrameLayout(this) }

    // Account tab and its screens.
    private val accountPage by lazy { FrameLayout(this) }
    private lateinit var computersRenderer: NativeRenderer
    private lateinit var tailnetRenderer: NativeRenderer
    private lateinit var account: AccountScreens
    private lateinit var playtest: Playtest
    private var routeBody: FrameLayout? = null
    private var routeBack: TextView? = null
    private var routeTitle: TextView? = null
    private var routeActions: LinearLayout? = null
    private var routeHome: FrameLayout? = null
    private var routeShared: View? = null
    private var shownHome: String? = null
    private var routeContent: FrameLayout? = null
    private var routeNotices: LinearLayout? = null
    private var routeQr: ImageView? = null
    private var shownQr: String? = null
    private var routeProgress: ProgressBar? = null
    private var routeInput: InputBar? = null

    // Verse tab controls.
    private lateinit var cameraButton: ImageButton
    private lateinit var recenterButton: ImageButton
    private lateinit var worldStatus: TextView
    private lateinit var worldRetry: TextView
    private lateinit var worldControls: LinearLayout
    private lateinit var panels: VersePanels

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
        bridge = MobileBridge(applicationContext, BuildConfig.DEBUG && intent.getBooleanExtra("computers_fixture", false),
            BuildConfig.DEBUG && intent.getBooleanExtra("wallet_fixture", false)) { render() }
        coderRenderer = NativeRenderer(this, { view, node -> bridge.activate("coder", view, node) },
            { token, value -> bridge.submit("coder", token, value) })
        computersRenderer = NativeRenderer(this, { view, node -> bridge.activate("computers", view, node) }, scrolling = true)
        account = AccountScreens(this, bridge)
        playtest = Playtest(this, bridge)
        payments = AgentPayments(this, bridge)
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
                // A long press on the tab bar reports the screen on view.
                setOnLongClickListener { report(); true }
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
            panels.setTopInset(bars.top)
            WindowInsetsCompat.CONSUMED
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                when {
                    tab == AppTab.ACCOUNT && route == AccountRoute.COMPUTERS && bridge.packet != null &&
                        bridge.packet?.objectOrNull("computers_home") == null -> bridge.computersGo("home")
                    tab == AppTab.ACCOUNT && route != null -> open(null)
                    tab == AppTab.VERSE && panels.showing -> panels.back()
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
        if (TranscriptDebug.ENABLED && intent.getBooleanExtra("rust_native_fixture", false)) {
            fixture = runCatching { JSONObject(assets.open("conversation.json").bufferedReader().readText()) }.getOrNull()
                ?.let { TranscriptFixture.prepare(it, intent.getIntExtra("rust_native_fixture_rows", 0),
                    intent.getBooleanExtra("rust_native_transcript_pull", false)) }
            // Streams a long reply into the fixture (not with a transcript source).
            val steps = intent.getIntExtra("rust_native_transcript_stream", 0)
            if (steps > 0 && !intent.getBooleanExtra("rust_native_transcript_pull", false)) streamFixture(0, steps)
        }
        select(tab)
        open(route)
        // Debug builds only: `--es coder_tap KEY[,KEY...]` taps Coder nodes in
        // order (a key ending in `*` taps the first whose key starts with the
        // rest), then `--es coder_send TEXT` sends TEXT from the composer.
        if (BuildConfig.DEBUG) {
            val taps = intent.getStringExtra("coder_tap")?.split(",").orEmpty().filter { it.isNotEmpty() }
            launchTaps(taps, intent.getStringExtra("coder_send"), 0)
        }
        tabBar.setOnLongClickListener { report(); true }
        // Debug builds only: `--ez report true` opens Report a problem for the first screen.
        if (BuildConfig.DEBUG && intent.getBooleanExtra("report", false)) main.postDelayed({ report() }, 1500)
        // Debug builds only: `--ez playtest_session true` turns the session on.
        if (BuildConfig.DEBUG && intent.getBooleanExtra("playtest_session", false)) {
            playtest.setSession(true, playtestTab, playtestRoute) { redrawAccountScreen() }
        }
    }

    // Playtest

    /** The tab as Rust's session log names it. */
    private val playtestTab get() = tab.name.lowercase()

    /** The screen as Rust's session log names it; an unnamed screen is its tab's `home`. */
    private val playtestRoute get() = when (tab) {
        AppTab.CODER -> if (bridge.packet?.optBoolean("terminal") == true) "terminal" else "home"
        AppTab.VERSE -> panels.openPanel.ifEmpty { "home" }
        AppTab.WALLET -> "home"
        AppTab.ACCOUNT -> route?.name?.lowercase() ?: "home"
    }

    /** Opens Report a problem for the screen on view. */
    fun report() = playtest.start(playtestTab, playtestRoute, if (tab == AppTab.VERSE) world else null)

    private fun screenChanged() { if (::playtest.isInitialized) bridge.playtestScreen(playtestTab, playtestRoute) }

    /** Debug builds only: levels come from the labeled tutorial fixture (`xp_preview`). */
    val xpPreview get() = BuildConfig.DEBUG && intent.getBooleanExtra("xp_preview", false)

    /** Debug builds only: Rust Native's sample conversation in place of the Coder surface. */
    private var fixture: JSONObject? = null

    /**
     * Debug builds only: adds a step of the fixture's streamed reply every
     * 60 ms from two seconds after launch, and logs the frame times while it
     * streams under `TranscriptStream`.
     */
    private fun streamFixture(step: Int, steps: Int) {
        val handler = android.os.Handler(mainLooper)
        val stats = TranscriptFrameStats("TranscriptStream", window.decorView)
        fun next(step: Int) {
            val view = fixture ?: return
            val more = TranscriptFixture.stream(view, step, steps)
            render()
            if (more) handler.postDelayed({ next(step + 1) }, 60) else handler.postDelayed({ stats.finish() }, 500)
        }
        handler.postDelayed({ stats.start(); next(step) }, 2_000)
    }

    // Tabs

    private fun select(value: AppTab) {
        tab = value
        for ((key, page) in pages) page.visibility = if (key == value) View.VISIBLE else View.GONE
        for ((key, button) in tabButtons) {
            button.alpha = if (key == value) 1f else 0.45f
            button.isSelected = key == value
        }
        world.setShown(value == AppTab.VERSE)
        if (value == AppTab.WALLET) wallet.appeared() else wallet.disappeared()
        bridge.coderShown(value == AppTab.CODER)
        if (value == AppTab.CODER && !bridge.busy) bridge.refreshComputers()
        screenChanged()
        render()
    }

    private fun buildWallet(page: FrameLayout) {
        wallet = WalletScreen(this, bridge, scanner)
        page.addView(wallet.root, FrameLayout.LayoutParams(-1, -1))
    }

    private fun buildVerse(page: FrameLayout) {
        val gymPreview = BuildConfig.DEBUG && intent.getBooleanExtra("gym_preview", false)
        world = VerseSurface(this, gymPreview, xpPreview) { packet, _ ->
            if (::panels.isInitialized) panels.update(packet)
            if (tab == AppTab.VERSE) renderVerse()
        }
        if (BuildConfig.DEBUG) intent.getStringExtra("verse_script")?.let { world.script = VerseScript.parse(it) }
        page.addView(world, FrameLayout.LayoutParams(-1, -1))
        panels = VersePanels(this, world)
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
        worldControls = controls
        page.addView(controls, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL).apply {
            setMargins(dp(16), dp(16), dp(16), dp(16)) })
        page.addView(panels.root, FrameLayout.LayoutParams(-1, -1))
    }

    private fun renderVerse() {
        val motion = world.snapshot?.optString("camera_mode") == "motion"
        cameraButton.contentDescription = if (motion) "Motion look" else "Touch look"
        cameraButton.setImageResource(if (motion) R.drawable.ic_motion_look else R.drawable.ic_touch_look)
        cameraButton.isEnabled = world.motionAvailable
        cameraButton.alpha = if (world.motionAvailable) 1f else 0.4f
        // The Gym and results panels show their own errors and hide the
        // camera controls.
        worldControls.visibility = if (panels.showing) View.GONE else View.VISIBLE
        val problem = world.nativeError ?: world.snapshot?.textOrNull("error") ?: world.motionError
        worldStatus.text = problem ?: ""
        worldStatus.visibility = if (problem == null) View.GONE else View.VISIBLE
        worldRetry.visibility = if (world.nativeError != null) View.VISIBLE else View.GONE
    }

    // Account

    private fun open(next: AccountRoute?) {
        if (next != route) routeInput?.dispose()
        if ((route == AccountRoute.IDENTITY || route == AccountRoute.TRAINER) && next != route) account.hideNsec()
        route = next
        accountPage.removeAllViews()
        routeContent = null; routeNotices = null; routeQr = null; shownQr = null
        routeProgress = null; routeInput = null; routeBody = null; routeBack = null; routeTitle = null
        routeActions = null; routeHome = null; routeShared = null; shownHome = null
        computersRenderer.clear(); tailnetRenderer.clear()
        if (next == null) {
            accountPage.addView(accountList(), FrameLayout.LayoutParams(-1, -1))
            if (tab == AppTab.ACCOUNT) screenChanged()
            render(); return
        }
        val screen = column()
        val header = row().apply {
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = dp(48)
        }
        routeBack = text("‹ Account", 17f).apply {
            setPadding(dp(12), dp(10), dp(16), dp(10)); contentDescription = "Back to Account"; tag = "account-back"
            setOnClickListener {
                if (route == AccountRoute.COMPUTERS && bridge.packet?.objectOrNull("computers_home") == null) bridge.computersGo("home")
                else open(null)
            }
        }
        header.addView(routeBack)
        // The Tailnet screen draws its own heading.
        routeTitle = text(if (next == AccountRoute.TAILNET) "" else next.title, 17f).bold().apply { gravity = Gravity.CENTER }
        header.addView(routeTitle, LinearLayout.LayoutParams(0, -2, 1f))
        routeActions = row().apply { gravity = Gravity.CENTER_VERTICAL; minimumWidth = dp(96) }
        header.addView(routeActions, LinearLayout.LayoutParams(-2, -2).apply { marginEnd = dp(8) })
        screen.addView(header)
        val body = FrameLayout(this)
        routeBody = body
        screen.addView(body, LinearLayout.LayoutParams(-1, 0, 1f))
        when (next) {
            AccountRoute.COMPUTERS -> {
                val column = column()
                routeNotices = column().also { column.addView(it) }
                val stack = FrameLayout(this)
                routeHome = FrameLayout(this).also { stack.addView(it, FrameLayout.LayoutParams(-1, -1)) }
                val content = FrameLayout(this)
                routeContent = content
                val shared = column()
                shared.addView(ScrollView(this).apply { addView(content) }, LinearLayout.LayoutParams(-1, 0, 1f))
                routeQr = ImageView(this).apply {
                    visibility = View.GONE; contentDescription = "Invitation QR code"; tag = "computers-qr"
                    scaleType = ImageView.ScaleType.FIT_CENTER
                }
                shared.addView(routeQr, LinearLayout.LayoutParams(dp(220), dp(220)).apply {
                    gravity = Gravity.CENTER_HORIZONTAL; setMargins(0, dp(12), 0, dp(12)) })
                routeShared = shared
                stack.addView(shared, FrameLayout.LayoutParams(-1, -1))
                column.addView(stack, LinearLayout.LayoutParams(-1, 0, 1f))
                routeInput = InputBar(this, scanner).also { input ->
                    column.addView(input.root, LinearLayout.LayoutParams(-1, -2).apply { setMargins(dp(8), dp(8), dp(8), dp(8)) })
                }
                body.addView(column, FrameLayout.LayoutParams(-1, -1))
            }
            AccountRoute.TAILNET -> {
                val column = column()
                routeNotices = column().also { column.addView(it) }
                val content = FrameLayout(this)
                routeContent = content
                val stack = FrameLayout(this)
                stack.addView(content, FrameLayout.LayoutParams(-1, -1))
                routeProgress = ProgressBar(this).apply { visibility = View.GONE; contentDescription = "Checking your tailnet" }
                stack.addView(routeProgress, FrameLayout.LayoutParams(dp(28), dp(28), Gravity.TOP or Gravity.END).apply {
                    setMargins(dp(16), dp(16), dp(16), dp(16)) })
                column.addView(stack, LinearLayout.LayoutParams(-1, 0, 1f))
                body.addView(column, FrameLayout.LayoutParams(-1, -1))
            }
            AccountRoute.IDENTITY, AccountRoute.DEVICE, AccountRoute.CHANGELOG -> {
                redrawAccountScreen()
                account.load { redrawAccountScreen() }
            }
            AccountRoute.TRAINER -> {
                redrawAccountScreen()
                account.loadTrainer(xpPreview) { redrawAccountScreen() }
            }
            AccountRoute.PLAYTEST -> {
                redrawAccountScreen()
                playtest.loadReports { redrawAccountScreen() }
                playtest.loadCard { redrawAccountScreen() }
            }
            AccountRoute.REPORTS -> {
                redrawAccountScreen()
                playtest.loadReports { redrawAccountScreen() }
            }
        }
        accountPage.addView(screen, FrameLayout.LayoutParams(-1, -1))
        if (next == AccountRoute.COMPUTERS) bridge.computersGo("home")
        if (next == AccountRoute.TAILNET) bridge.snapshot()
        if (tab == AppTab.ACCOUNT) screenChanged()
        render()
    }

    /** Rebuilds the Identity keys, About this device, or Changelog screen. */
    private fun redrawAccountScreen() {
        val body = routeBody ?: return
        val view = when (route) {
            AccountRoute.IDENTITY -> account.identity { redrawAccountScreen() }
            AccountRoute.TRAINER -> account.trainer(xpPreview) { redrawAccountScreen() }
            AccountRoute.DEVICE -> account.about(bridge.packet)
            AccountRoute.CHANGELOG -> account.changelog()
            AccountRoute.PLAYTEST -> playtest.screen({ redrawAccountScreen() }, { report() }, { open(AccountRoute.REPORTS) })
            AccountRoute.REPORTS -> playtest.myReports { report() }
            else -> return
        }
        body.removeAllViews()
        body.addView(view, FrameLayout.LayoutParams(-1, -1))
    }

    private fun accountList(): View = ScrollView(this).apply {
        addView(column().apply {
            setPadding(dp(16), dp(12), dp(16), dp(24))
            addView(text("Account", 32f).bold(), LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(16) })
            addView(group(listOf(AccountRoute.TRAINER).map { "★  ${it.title}" to "account-trainer" to { open(it) } }))
            addView(group(listOf(
                "Playtest" to "account-playtest" to { open(AccountRoute.PLAYTEST) },
                "Report a problem" to "account-report" to { report() },
            )), LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
            addView(group(listOf(AccountRoute.COMPUTERS, AccountRoute.TAILNET).map { it.title to "account-${it.name.lowercase()}" to { open(it) } }))
            addView(group(listOf(AccountRoute.IDENTITY, AccountRoute.DEVICE, AccountRoute.CHANGELOG).map {
                it.title to "account-${it.name.lowercase()}" to { open(it) } }),
                LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
            addView(group(listOf(
                "Source code" to "account-source" to { browse("https://github.com/OpenAgentsInc/openagents") },
                "Follow us on X" to "account-x" to { browse("https://x.com/OpenAgentsInc") },
            ), external = true), LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
        })
    }

    /** Opens an https page in the browser. */
    fun openLink(link: String) = browse(link)

    private fun browse(link: String) {
        try { startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(link))) }
        catch (_: android.content.ActivityNotFoundException) {}
    }

    private fun group(rows: List<Pair<Pair<String, String>, () -> Unit>>, external: Boolean = false): View = column().apply {
        background = rounded(Palette.SURFACE, 12f)
        rows.forEachIndexed { index, (labels, action) ->
            val (title, key) = labels
            if (index > 0) addView(View(this@MainActivity).apply { setBackgroundColor(Palette.BORDER) },
                LinearLayout.LayoutParams(-1, 1).apply { marginStart = dp(16) })
            addView(row().apply {
                gravity = Gravity.CENTER_VERTICAL
                minimumHeight = dp(52)
                setPadding(dp(16), 0, dp(16), 0)
                tag = key
                addView(text(title, 17f), LinearLayout.LayoutParams(0, -2, 1f))
                addView(text(if (external) "↗" else "›", if (external) 17f else 22f, Palette.TERTIARY))
                contentDescription = if (external) "$title, opens in the browser" else title
                setOnClickListener { action() }
            }, LinearLayout.LayoutParams(-1, -2))
        }
    }

    /** The Computers header: the list's menu and Add, or back to the list past it. */
    private fun computersHeader(home: JSONObject?) {
        val actions = routeActions ?: return
        routeBack?.text = if (home == null && bridge.packet != null) "‹ Computers" else "‹ Account"
        routeBack?.contentDescription = if (home == null && bridge.packet != null) "Back to Computers" else "Back to Account"
        routeTitle?.text = if (home != null) "Computers" else ""
        val wanted = if (home != null) "list" else "none"
        if (actions.tag == wanted) return
        actions.tag = wanted
        actions.removeAllViews()
        if (home == null) return
        actions.addView(text("⋯", 22f).apply {
            gravity = Gravity.CENTER; contentDescription = "More"; tag = "computers-more"
            setOnClickListener { v -> bridge.packet?.objectOrNull("computers_home")?.let { account.listMenu(v, it) } }
        }, LinearLayout.LayoutParams(dp(44), dp(44)))
        actions.addView(text("+", 26f).apply {
            gravity = Gravity.CENTER; contentDescription = "Add a computer"; tag = "computers-add"
            setOnClickListener { bridge.computersGo("add") }
        }, LinearLayout.LayoutParams(dp(44), dp(44)))
    }

    private fun launchTaps(taps: List<String>, send: String?, attempt: Int) {
        if (taps.isEmpty() && send == null || attempt > 40) return
        main.postDelayed({
            val view = bridge.packet?.objectOrNull("coder")
            val root = view?.optJSONObject("root")
            if (view == null || root == null || bridge.busy) { launchTaps(taps, send, attempt + 1); return@postDelayed }
            if (taps.isNotEmpty()) {
                val key = taps.first()
                val found = findNode(root) { node ->
                    val name = node.optString("key")
                    if (key.endsWith("*")) name.startsWith(key.dropLast(1)) else name == key
                }
                if (found == null) { launchTaps(taps, send, attempt + 1); return@postDelayed }
                bridge.activate("coder", view, found.optString("key"))
                launchTaps(taps.drop(1), send, 0)
            } else if (send != null) {
                val composer = findNode(root) { it.optJSONObject("element")?.optString("kind") == "composer" }
                if (composer == null) { launchTaps(taps, send, attempt + 1); return@postDelayed }
                val token = composer.getJSONObject("element").getJSONObject("props").getString("token")
                bridge.submit("coder", token, send)
            }
        }, 500)
    }

    private fun findNode(node: JSONObject, matches: (JSONObject) -> Boolean): JSONObject? {
        if (matches(node)) return node
        val children = node.optJSONObject("element")?.optJSONObject("props")?.optJSONArray("children") ?: return null
        for (index in 0 until children.length()) {
            children.optJSONObject(index)?.let { child -> findNode(child, matches)?.let { return it } }
        }
        return null
    }

    // Rendering

    private fun render() {
        if (!::bridge.isInitialized) return
        // A chat asked to connect a computer: Account > Computers.
        if (bridge.computersRequested != computersShown) {
            computersShown = bridge.computersRequested
            select(AppTab.ACCOUNT)
            open(AccountRoute.COMPUTERS)
        }
        val packet = bridge.packet
        mount(coderRenderer, coderContent, fixture ?: packet?.objectOrNull("coder"))
        when (route) {
            AccountRoute.COMPUTERS -> {
                val home = packet?.objectOrNull("computers_home")
                computersHeader(home)
                routeHome?.visibility = if (home != null) View.VISIBLE else View.GONE
                routeShared?.visibility = if (home != null) View.GONE else View.VISIBLE
                if (home != null) {
                    val encoded = home.toString()
                    if (encoded != shownHome) {
                        shownHome = encoded
                        routeHome?.let { it.removeAllViews(); it.addView(account.computers(home), FrameLayout.LayoutParams(-1, -1)) }
                    }
                    computersRenderer.clear(); routeContent?.removeAllViews()
                } else {
                    shownHome = null
                    routeContent?.let { mount(computersRenderer, it, packet?.objectOrNull("computers")) }
                }
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
            AccountRoute.TAILNET -> {
                routeContent?.let { mount(tailnetRenderer, it, packet?.objectOrNull("tailnet")) }
                notices(packet, false)
                routeProgress?.visibility = if (packet?.optBoolean("tailnet_loading") == true) View.VISIBLE else View.GONE
            }
            AccountRoute.DEVICE -> if (routeBody?.findViewWithTag<View>("device-key")?.contentDescription != packet?.textOrNull("device")) redrawAccountScreen()
            AccountRoute.IDENTITY, AccountRoute.CHANGELOG, AccountRoute.TRAINER, AccountRoute.PLAYTEST, AccountRoute.REPORTS -> Unit
            null -> Unit
        }
        if (tab == AppTab.WALLET) wallet.update(packet)
        // An agent's payment request shows over any tab until the owner
        // approves or denies it; Rust closes it.
        if (::payments.isInitialized) payments.update(packet)
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
            // Rust says when a Coder chat changes and asks every second while
            // one runs (MobileBridge.watchChanges); this is only a fallback.
            if (ticks % 3 == 0 && computersPolling && !bridge.busy) bridge.refreshComputers()
            val loading = tab == AppTab.ACCOUNT && (
                (route == AccountRoute.TAILNET && packet?.optBoolean("tailnet_loading") == true))
            if (loading && bridge.pending < 2) bridge.snapshot()
            // The trainer card reads awards from the relay; refresh it.
            if (tab == AppTab.ACCOUNT && route == AccountRoute.TRAINER && ticks % 2 == 0 && !xpPreview && account.nsec == null) {
                account.loadTrainer(false) { if (route == AccountRoute.TRAINER) redrawAccountScreen() }
            }
            // A report in flight turns Sent or Not sent yet on its own.
            if (tab == AppTab.ACCOUNT && route == AccountRoute.REPORTS && playtest.sending && ticks % 2 == 0) {
                playtest.loadReports { if (route == AccountRoute.REPORTS) redrawAccountScreen() }
            }
            // The playtest card reads awards from the playtest referee.
            if (tab == AppTab.ACCOUNT && route == AccountRoute.PLAYTEST && ticks % 3 == 0 && !xpPreview) {
                playtest.loadCard { if (route == AccountRoute.PLAYTEST) redrawAccountScreen() }
            }
            // Starts, syncs, quotes, and payments finish in the background:
            // poll every second while one runs, else every ten seconds for
            // payments that arrive.
            val walletLoading = packet?.optBoolean("wallet_loading") == true
            if (tab == AppTab.WALLET && (walletLoading || ticks % 10 == 0) && !bridge.busy) bridge.snapshot()
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
        // The nsec shows only while its screen is open and in front.
        if (account.nsec != null) {
            account.hideNsec()
            if (route == AccountRoute.IDENTITY || route == AccountRoute.TRAINER) redrawAccountScreen()
        }
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
