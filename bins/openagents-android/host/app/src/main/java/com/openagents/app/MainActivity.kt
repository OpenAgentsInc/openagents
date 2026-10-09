// Rust builds every screen; this host renders them and supplies the pieces a
// Rust Native tree cannot: tabs, navigation, the camera, keyboards, the
// terminal's keyboard target, and the Verse surface.
package com.openagents.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.drawable.BitmapDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.graphics.Rect
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
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
 * The app's places. The drawer (#11126) opens each; the chat is the home
 * place, and Settings holds the screens that used to be tabs.
 */
enum class AppTab(val title: String) {
    CODER("Chat"),
    VERSE("Verse"),
    WALLET("Wallet"),
    ACCOUNT("Settings"),
}

/** A screen that the Account tab opens. */
enum class AccountRoute(val title: String) {
    TRAINER("Trainer"), COMPUTERS("Computers"), TAILNET("Tailnet"), KEYS("Your keys"), IDENTITY("Identity keys"), DEVICE("About this device"), CHANGELOG("Changelog"),
    PLAYTEST("Playtest"), REPORTS("My reports");

    /** Shown only in a preview build ([Preview]). */
    val previewOnly: Boolean get() = this == TRAINER || this == TAILNET || this == PLAYTEST || this == REPORTS
}

/**
 * The release gate (docs/mobile/1.0-audit.md): the Verse tab, the Gym
 * (Train Coder, Profile), Trainer, Playtest, Tailnet, and the display name
 * show only in a build whose Rust library was made with
 * `OPENAGENTS_MOBILE_PREVIEW=on`. Release and normal debug builds hide them.
 */
object Preview {
    val on: Boolean by lazy { runCatching { OpenAgentsNative.preview() }.getOrDefault(false) }
    fun shows(tab: AppTab) = on || tab != AppTab.VERSE
    fun shows(route: AccountRoute) = on || !route.previewOnly
}

class MainActivity : ComponentActivity() {
    /** A tap anywhere outside the focused text field, on any screen, puts the
     *  keyboard away. The tap still reaches what it hit, so a tab tapped with
     *  the keyboard up still switches. */
    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_DOWN) {
            val focus = currentFocus
            if (focus is EditText) {
                val bounds = Rect()
                focus.getGlobalVisibleRect(bounds)
                if (!bounds.contains(event.rawX.toInt(), event.rawY.toInt())) {
                    getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(focus.windowToken, 0)
                    focus.clearFocus()
                }
            }
        }
        return super.dispatchTouchEvent(event)
    }

    private lateinit var bridge: MobileBridge
    private lateinit var scanner: QRScanner
    private lateinit var world: VerseSurface
    private lateinit var terminal: TerminalScreen
    private lateinit var worldTerminalKeys: TerminalKeyView
    private var worldTerminalFeed: String? = null
    private lateinit var connect: ConnectScreen
    private val main = Handler(Looper.getMainLooper())
    private var tab = AppTab.CODER
    private var route: AccountRoute? = null
    /** The Your keys section last drawn, and the add-key asks already shown. */
    private var shownKeys: String? = null
    private var askMineShown = 0
    private var resumed = false
    private var ticks = 0
    /** The Coder tab's requests to open Account > Computers, as last handled. */
    private var computersShown = 0
    private var screensShown = 0
    private var imagePicksShown = 0
    /** Each attached image's card by resource, so a refresh keeps it. */
    private val imageViews = HashMap<String, View>()

    /**
     * **Attach image**: the system photo picker; Rust decodes the photo. It
     * opens only while the chat takes images; the phone is text only since
     * #10093 ([PhoneAttachments]).
     */
    private val pickImage = registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) Thread {
            val bytes = runCatching {
                contentResolver.openInputStream(uri)?.use { input -> input.readBytes() }
            }.getOrNull()
            val photo = bytes?.let { ChatImages.encoded(it) }
            main.post { photo?.let { (name, data) -> bridge.attachImage(name, data) } }
        }.start()
    }
    private var statusTop = 0

    private lateinit var root: FrameLayout
    private val pages = mutableMapOf<AppTab, FrameLayout>()

    // The shell (#11126): the chat's top bar, the drawer, and the new
    // chat's feature cards. Rust's `shell` packet says what they show.
    private lateinit var body: LinearLayout
    private lateinit var shellBar: ShellTopBar
    private lateinit var drawer: ShellDrawer
    private lateinit var scrim: View
    private lateinit var homeCards: HomeCards
    private var drawerOpen = false
    private val walletMenu by lazy { FrameLayout(this) }
    private val verseMenu by lazy { FrameLayout(this) }

    private lateinit var wallet: WalletScreen
    private lateinit var payments: AgentPayments
    private lateinit var walletLink: WalletLink

    // Coder tab: the menu, the first run, or the chat (Rust's `gym.screen`).
    private lateinit var coderRenderer: NativeRenderer
    private val coderContent by lazy { FrameLayout(this) }
    private val gymContent by lazy { FrameLayout(this) }
    private lateinit var gym: GymViews
    private var shownGym: String? = null

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
    private lateinit var studio: VerseStudio

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

    /** Each Gym card's view by ID, with the content it was built from. */
    private val cardViews = HashMap<String, Pair<String, View>>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        // The theme Rust resolved last time, so the first frame is drawn in
        // it; Rust's first packet confirms or changes it.
        Palette.apply(savedAppearance())
        restyleWindow()
        scanner = QRScanner(this)
        bridge = MobileBridge(applicationContext, BuildConfig.DEBUG && intent.getBooleanExtra("computers_fixture", false),
            BuildConfig.DEBUG && intent.getBooleanExtra("wallet_fixture", false),
            BuildConfig.DEBUG && intent.getBooleanExtra("chat_fixture", false),
            BuildConfig.DEBUG && intent.getBooleanExtra("gym_fixture", false)) { render() }
        bridge.systemAppearance(night(resources.configuration))
        gym = GymViews(this) { id -> bridge.gym(id) }
        homeCards = HomeCards(this) { id -> bridge.shell("try_card", "id" to id) }
        coderRenderer = NativeRenderer(this, { view, node -> bridge.activate("coder", view, node) },
            { token, value -> bridge.submit("coder", token, value) }, floating = true, surfaces = { resource ->
                if (resource == "home-cards") homeCards.root
                else if (resource.startsWith("link:")) LinkCards.card(this, bridge, resource) { openLink(it) }
                else if (resource.startsWith("image:")) imageViews.getOrPut(resource) { ChatImages.card(this, bridge, resource) }
                else resource.removePrefix("gym-card:").takeIf { it != resource }?.let { id ->
                    bridge.packet?.objectOrNull("gym")?.objectOrNull("cards")?.objectOrNull(id)?.let { card ->
                        // Rebuild a card only when its content changes: the
                        // packet refreshes every second while a chat shows,
                        // and a new view each time reset its scroll mid-drag.
                        val encoded = card.toString()
                        cardViews[id]?.takeIf { it.first == encoded }?.second
                            ?: gym.cappedCard(card).also { cardViews[id] = encoded to it }
                    }
                }
            })
        computersRenderer = NativeRenderer(this, { view, node -> bridge.activate("computers", view, node) }, scrolling = true)
        account = AccountScreens(this, bridge)
        playtest = Playtest(this, bridge)
        SelectionLayer.giveFeedback = { text, row -> playtest.feedback(text, row, playtestTab, playtestRoute) }
        payments = AgentPayments(this, bridge)
        walletLink = WalletLink(this, bridge)
        tailnetRenderer = NativeRenderer(this, { view, node -> bridge.activate("tailnet", view, node) })
        terminal = TerminalScreen(this, bridge)
        connect = ConnectScreen(this, bridge, scanner)

        root = FrameLayout(this).apply { setBackgroundColor(Palette.BACKGROUND) }
        body = column().apply { setBackgroundColor(Palette.BACKGROUND) }
        val pageHost = FrameLayout(this)
        body.addView(pageHost, LinearLayout.LayoutParams(-1, 0, 1f))
        // The Verse's page is built, but the drawer offers it only in a preview build.
        for (value in AppTab.entries) {
            val page = FrameLayout(this).apply { visibility = View.GONE }
            pages[value] = page
            pageHost.addView(page, FrameLayout.LayoutParams(-1, -1))
        }
        shellBar = ShellTopBar(this, bridge, { openDrawer(true) }) { report() }
        val coderPage = column()
        coderPage.addView(shellBar.root, LinearLayout.LayoutParams(-1, -2))
        val coderStack = FrameLayout(this)
        coderStack.addView(coderContent, FrameLayout.LayoutParams(-1, -1))
        coderStack.addView(gymContent, FrameLayout.LayoutParams(-1, -1))
        coderPage.addView(coderStack, LinearLayout.LayoutParams(-1, 0, 1f))
        pages.getValue(AppTab.CODER).addView(coderPage, FrameLayout.LayoutParams(-1, -1))
        buildVerse(pages.getValue(AppTab.VERSE))
        val walletPage = column()
        walletPage.addView(walletMenu, LinearLayout.LayoutParams(-1, -2))
        val walletBody = FrameLayout(this)
        walletPage.addView(walletBody, LinearLayout.LayoutParams(-1, 0, 1f))
        pages.getValue(AppTab.WALLET).addView(walletPage, FrameLayout.LayoutParams(-1, -1))
        buildWallet(walletBody)
        pages.getValue(AppTab.VERSE).addView(verseMenu, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.START))
        pages.getValue(AppTab.ACCOUNT).addView(accountPage, FrameLayout.LayoutParams(-1, -1))
        // The drawer lies under the app; opening it slides the app aside.
        drawer = ShellDrawer(this, bridge) { place -> go(place) }
        drawer.root.visibility = View.GONE
        root.addView(drawer.root, FrameLayout.LayoutParams(drawerWidth(), -1))
        root.addView(body, FrameLayout.LayoutParams(-1, -1))
        scrim = View(this).apply {
            visibility = View.GONE
            contentDescription = "Close menu"; tag = "shell-close"
            setOnClickListener { openDrawer(false) }
        }
        root.addView(scrim, FrameLayout.LayoutParams(-1, -1))
        menus()
        root.addView(terminal.root, FrameLayout.LayoutParams(-1, -1))
        // Connect a computer shows over every tab.
        root.addView(connect.root, FrameLayout.LayoutParams(-1, -1))
        setContentView(root)

        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            val typing = insets.isVisible(WindowInsetsCompat.Type.ime())
            statusTop = bars.top
            body.setPadding(bars.left, 0, bars.right, maxOf(bars.bottom, keyboard.bottom))
            drawer.root.setPadding(bars.left, bars.top, 0, maxOf(bars.bottom, keyboard.bottom))
            terminal.root.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            connect.root.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            verseMenu.setPadding(dp(16), bars.top + dp(4), 0, 0)
            // Every page but Verse starts below the status bar; the world
            // fills the screen behind it and keeps its controls below.
            for ((value, page) in pages) if (value != AppTab.VERSE) page.setPadding(0, bars.top, 0, 0)
            val density = resources.displayMetrics.density
            world.setHudInsets(bars.top / density, 0f, 0f, 0f)
            panels.setTopInset(bars.top)
            studio.setTopInset(bars.top)
            WindowInsetsCompat.CONSUMED
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                when {
                    drawerOpen -> openDrawer(false)
                    connect.showing -> connect.back()
                    tab == AppTab.ACCOUNT && route == AccountRoute.COMPUTERS && bridge.packet != null &&
                        bridge.packet?.objectOrNull("computers_home") == null -> bridge.computersGo("home")
                    tab == AppTab.ACCOUNT && route != null -> open(null)
                    tab == AppTab.VERSE && panels.showing -> panels.back()
                    tab == AppTab.VERSE && studio.showing -> studio.back()
                    tab != AppTab.CODER -> select(AppTab.CODER)
                    else -> { isEnabled = false; onBackPressedDispatcher.onBackPressed(); isEnabled = true }
                }
            }
        })

        // Developer launch extras open a tab or an Account screen directly,
        // for example `--es tab account --es account_route tailnet`.
        if (BuildConfig.DEBUG) {
            intent.getStringExtra("tab")?.let { name -> AppTab.entries.firstOrNull { it.name.equals(name, true) } }
                ?.takeIf { Preview.shows(it) }?.let { tab = it }
            intent.getStringExtra("account_route")?.let { name -> AccountRoute.entries.firstOrNull { it.name.equals(name, true) } }
                ?.takeIf { Preview.shows(it) }?.let { tab = AppTab.ACCOUNT; route = it }
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
        if (savedInstanceState == null) handleLink(intent)
        // Debug builds only: `--es coder_tap KEY[,KEY...]` taps Coder nodes in
        // order (a key ending in `*` taps the first whose key starts with the
        // rest), then `--es coder_send TEXT` sends TEXT from the composer.
        if (BuildConfig.DEBUG) {
            val taps = intent.getStringExtra("coder_tap")?.split(",").orEmpty().filter { it.isNotEmpty() }
            launchTaps(taps, intent.getStringExtra("coder_send"), 0)
            // `--es attach_image NAME` hands NAME from the app's external
            // files directory to the draft, as the photo picker would
            // (dropped while the chat is text only, #10093).
            intent.getStringExtra("attach_image")?.let { name ->
                main.postDelayed({
                    val file = java.io.File(getExternalFilesDir(null), name)
                    runCatching { file.readBytes() }.getOrNull()?.let { ChatImages.encoded(it) }
                        ?.let { (photo, data) -> bridge.attachImage(photo, data) }
                }, 3000)
            }
            intent.getStringExtra("gym_script")?.let { script -> gymScript(script.split("|").filter { it.isNotEmpty() }, 0) }
        }
        // Debug builds only, for screenshots: `--es shell_mode code` opens
        // Code mode, `--ez drawer true` opens the drawer, and `--es
        // appearance light|dark|system` picks the theme (saved).
        if (BuildConfig.DEBUG) {
            if (intent.getStringExtra("shell_mode") == "code") bridge.shell("mode", "code" to true)
            intent.getStringExtra("appearance")?.let { bridge.theme(it) }
            if (intent.getBooleanExtra("drawer", false)) main.postDelayed({ openDrawer(true) }, 1500)
        }
        // Debug builds only: `--ez report true` opens Report a problem for the first screen.
        if (BuildConfig.DEBUG && intent.getBooleanExtra("report", false)) main.postDelayed({ report() }, 1500)
    }

    // Playtest

    /** The tab as Rust's playtest log names it. */
    private val playtestTab get() = tab.name.lowercase()

    /** The screen as Rust's playtest log names it; an unnamed screen is its tab's `home`. */
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
        if (value != tab) currentFocus?.let { focus ->
            // Another tab: the chat's keyboard goes away with it.
            getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(focus.windowToken, 0)
            focus.clearFocus()
        }
        tab = value
        for ((key, page) in pages) page.visibility = if (key == value) View.VISIBLE else View.GONE
        world.setShown(value == AppTab.VERSE)
        if (value == AppTab.WALLET) wallet.appeared() else wallet.disappeared()
        bridge.coderShown(value == AppTab.CODER)
        if (value == AppTab.CODER && !bridge.busy) bridge.refreshComputers()
        screenChanged()
        render()
    }

    // The shell (#11126)

    /** The drawer's width: most of the screen, at most 360 dp. */
    private fun drawerWidth() = minOf((resources.displayMetrics.widthPixels * 0.82f).toInt(), dp(360))

    /** The menu buttons over the Wallet and the Verse, which have no top bar of their own. */
    private fun menus() {
        walletMenu.removeAllViews()
        walletMenu.setPadding(dp(16), dp(4), dp(16), dp(4))
        walletMenu.setBackgroundColor(Palette.BACKGROUND)
        walletMenu.addView(menuButton({ openDrawer(true) }) { report() }, FrameLayout.LayoutParams(dp(44), dp(44)))
        verseMenu.removeAllViews()
        if (Preview.on) verseMenu.addView(menuButton({ openDrawer(true) }) { report() }, FrameLayout.LayoutParams(dp(44), dp(44)))
    }

    /** Opens or closes the drawer, sliding the app aside; Rust lists its chats while it is open. */
    private fun openDrawer(open: Boolean) {
        if (open == drawerOpen) return
        drawerOpen = open
        bridge.shell("drawer", "open" to open)
        currentFocus?.let { focus ->
            getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(focus.windowToken, 0)
            focus.clearFocus()
        }
        val width = drawerWidth()
        drawer.root.layoutParams = (drawer.root.layoutParams as FrameLayout.LayoutParams).apply { this.width = width }
        if (open) {
            drawer.update(bridge.packet?.objectOrNull("shell"))
            drawer.root.visibility = View.VISIBLE
            scrim.visibility = View.VISIBLE
            scrim.alpha = 0f
            scrim.setBackgroundColor(if (Palette.LIGHT) 0x1F000000 else 0x59000000)
        } else drawer.closed()
        // The app beside the drawer is hidden from TalkBack while it is open.
        body.importantForAccessibility = if (open) View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            else View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        val to = if (open) width.toFloat() else 0f
        val duration = if (android.animation.ValueAnimator.areAnimatorsEnabled()) 250L else 0L
        body.animate().translationX(to).setDuration(duration).start()
        scrim.animate().translationX(to).alpha(if (open) 1f else 0f).setDuration(duration).withEndAction {
            if (!drawerOpen) { scrim.visibility = View.GONE; drawer.root.visibility = View.GONE }
        }.start()
    }

    /** Opens a place from the drawer. */
    private fun go(place: String) {
        when (place) {
            "code" -> {
                bridge.shell("mode", "code" to true)
                bridge.shell("new_chat")
                select(AppTab.CODER)
            }
            "computers" -> { select(AppTab.ACCOUNT); open(AccountRoute.COMPUTERS) }
            "wallet" -> select(AppTab.WALLET)
            "verse" -> if (Preview.on) select(AppTab.VERSE)
            "settings" -> { select(AppTab.ACCOUNT); open(null) }
            else -> select(AppTab.CODER)
        }
        openDrawer(false)
    }

    private fun buildWallet(page: FrameLayout) {
        wallet = WalletScreen(this, bridge, scanner)
        page.addView(wallet.root, FrameLayout.LayoutParams(-1, -1))
    }

    private fun buildVerse(page: FrameLayout) {
        val gymPreview = BuildConfig.DEBUG && intent.getBooleanExtra("gym_preview", false)
        world = VerseSurface(this, gymPreview, xpPreview) { packet, _ ->
            if (::panels.isInitialized) panels.update(packet)
            if (::studio.isInitialized) studio.update(packet)
            if (tab == AppTab.VERSE) renderVerse()
        }
        if (BuildConfig.DEBUG) intent.getStringExtra("verse_script")?.let { world.script = VerseScript.parse(it) }
        page.addView(world, FrameLayout.LayoutParams(-1, -1))
        worldTerminalKeys = TerminalKeyView(this,
            text = { bridge.terminal(json("op" to "terminal_text", "text" to it)) },
            key = { name, ctrl, alt, shift -> bridge.terminal(json("op" to "terminal_key", "key" to name,
                "ctrl" to ctrl, "alt" to alt, "shift" to shift)) })
        page.addView(worldTerminalKeys, FrameLayout.LayoutParams(1, 1))
        world.computerCommands = { commands ->
            for (i in 0 until commands.length()) {
                val command = commands.optJSONObject(i) ?: continue
                when (command.optString("kind")) {
                    "activate" -> bridge.activate(command.optString("surface"),
                        json("instance" to command.optString("instance"), "revision" to command.optLong("revision")),
                        command.optString("node"))
                    "refresh" -> bridge.refreshComputers()
                    "terminal_resize" -> bridge.terminal(json("op" to "terminal_resize",
                        "rows" to command.optInt("rows"), "cols" to command.optInt("cols")))
                    "copy" -> getSystemService(android.content.ClipboardManager::class.java)
                        ?.setPrimaryClip(android.content.ClipData.newPlainText("Terminal selection", command.optString("text")))
                    "terminal_keyboard" -> {
                        worldTerminalKeys.requestFocus()
                        getSystemService(android.view.inputmethod.InputMethodManager::class.java)
                            ?.showSoftInput(worldTerminalKeys, 0)
                    }
                    "type", "scan" -> { select(AppTab.ACCOUNT); open(AccountRoute.COMPUTERS) }
                    "cancel_input" -> bridge.cancel("computers", command.optString("token"))
                }
            }
        }

        panels = VersePanels(this, world)
        panels.onTrain = { bridge.gymTrain() }
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
        // Everglade's studio: its connection line, Interact, and panel.
        studio = VerseStudio(this, world, { bridge.studioComputer() }) { host, done -> bridge.studioLinks(host, done) }
        page.addView(studio.root, FrameLayout.LayoutParams(-1, -1))
        page.addView(panels.root, FrameLayout.LayoutParams(-1, -1))
    }

    private fun renderVerse() {
        val computerOpen = world.snapshot?.optBoolean("computer_open") == true
        if (computerOpen) {
            val feed = json("computers" to bridge.packet?.optJSONObject("computers"),
                "terminal" to bridge.terminalView)
            val encoded = feed.toString()
            if (encoded != worldTerminalFeed) {
                worldTerminalFeed = encoded
                world.send(json("action" to "computer_feed", "feed" to feed), false)
            }
            if (bridge.packet?.optBoolean("terminal") == true) bridge.pollTerminal()
        } else {
            worldTerminalFeed = null
            if (::worldTerminalKeys.isInitialized) worldTerminalKeys.clearFocus()
        }

        val motion = world.snapshot?.optString("camera_mode") == "motion"
        cameraButton.contentDescription = if (motion) "Motion look" else "Touch look"
        cameraButton.setImageResource(if (motion) R.drawable.ic_motion_look else R.drawable.ic_touch_look)
        cameraButton.isEnabled = world.motionAvailable
        cameraButton.alpha = if (world.motionAvailable) 1f else 0.4f
        // The Gym and results panels show their own errors and hide the
        // camera controls.
        worldControls.visibility = if (panels.showing || studio.showing) View.GONE else View.VISIBLE
        studio.setBoardOpen(panels.showing)
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
        routeBack = text("‹ Settings", 17f).apply {
            setPadding(dp(12), dp(10), dp(16), dp(10)); contentDescription = "Back to Settings"; tag = "account-back"
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
            AccountRoute.KEYS -> redrawAccountScreen()
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
            AccountRoute.KEYS -> {
                shownKeys = bridge.packet?.objectOrNull("provider_keys")?.toString() + bridge.providerKeyError
                account.yourKeys(bridge.packet?.objectOrNull("provider_keys")) { redrawAccountScreen() }
            }
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
            addView(row().apply {
                gravity = Gravity.CENTER_VERTICAL
                addView(menuButton({ openDrawer(true) }) { report() }, LinearLayout.LayoutParams(dp(44), dp(44)))
                addView(text("Settings", 32f).bold(), LinearLayout.LayoutParams(-1, -2).apply { marginStart = dp(14) })
            }, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(16) })
            if (Preview.on) addView(group(listOf(
                "★  ${AccountRoute.TRAINER.title}" to "account-trainer" to { open(AccountRoute.TRAINER) },
                // Train Coder opened a test of the Gym's sample plugins, which are no longer shown; it comes back when the Gym has a real plugin to test.
                // "Train Coder" to "account-train" to { bridge.gymTrain() },
                // Profile: Rust shows it as a sheet on the Chat tab.
                "Profile" to "account-profile" to { bridge.profile() },
            )))
            addView(group(listOfNotNull(
                ("Playtest" to "account-playtest" to { open(AccountRoute.PLAYTEST) }).takeIf { Preview.on },
                "Report a problem" to "account-report" to { report() },
            )), LinearLayout.LayoutParams(-1, -2).apply { if (Preview.on) topMargin = dp(24) })
            addView(group(listOf(AccountRoute.COMPUTERS, AccountRoute.TAILNET).filter { Preview.shows(it) }
                .map { it.title to "account-${it.name.lowercase()}" to { open(it) } }))
            // System follows the phone; Rust saves the choice.
            addView(group(listOf("Appearance" to "account-appearance" to { chooseTheme() })),
                LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
            addView(group(listOf(AccountRoute.KEYS, AccountRoute.IDENTITY, AccountRoute.DEVICE, AccountRoute.CHANGELOG).map {
                it.title to "account-${it.name.lowercase()}" to { open(it) } }),
                LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
            addView(group(listOf(
                "Source code" to "account-source" to { browse("https://github.com/OpenAgentsInc/openagents") },
                "Follow us on X" to "account-x" to { browse("https://x.com/OpenAgentsInc") },
            ), external = true), LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(24) })
        })
    }

    /** Account > Appearance: System, Light, or Dark, as Rust lists them. */
    private fun chooseTheme() {
        val appearance = bridge.packet?.objectOrNull("appearance") ?: return
        val choices = appearance.optJSONArray("choices")?.objects() ?: return
        val checked = choices.indexOfFirst { it.optBoolean("selected") }
        android.app.AlertDialog.Builder(this)
            .setTitle("Appearance")
            .setSingleChoiceItems(choices.map { it.optString("label") }.toTypedArray(), checked) { dialog, index ->
                bridge.theme(choices[index].optString("id"))
                dialog.dismiss()
            }
            .setNegativeButton("Cancel", null)
            .show()
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
        routeBack?.text = if (home == null && bridge.packet != null) "‹ Computers" else "‹ Settings"
        routeBack?.contentDescription = if (home == null && bridge.packet != null) "Back to Computers" else "Back to Settings"
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

    /**
     * Debug builds only: `--es gym_script "tap:first.choose|send:TEXT|tap:*.start|sleep:3"`.
     * Each step waits up to a minute for its Gym button, chat node, or composer;
     * `*` ends a prefix, or starts a suffix.
     */
    private fun gymScript(steps: List<String>, attempt: Int) {
        if (steps.isEmpty() || attempt > 120) return
        val (verb, value) = steps.first().split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        if (verb == "sleep") { main.postDelayed({ gymScript(steps.drop(1), 0) }, ((value.toDoubleOrNull() ?: 1.0) * 1000).toLong()); return }
        main.postDelayed({
            fun matches(id: String) = when {
                value.startsWith("*") -> id.endsWith(value.drop(1))
                value.endsWith("*") -> id.startsWith(value.dropLast(1))
                else -> id == value
            }
            val gymPacket = bridge.packet?.objectOrNull("gym")
            val view = bridge.packet?.objectOrNull("coder")
            val root = view?.optJSONObject("root")
            val done = when {
                bridge.busy -> false
                verb == "send" -> root?.let { findNode(it) { n -> n.optJSONObject("element")?.optString("kind") == "composer" } }
                    ?.takeIf { gymPacket?.optString("screen") == "chat" }
                    ?.let { bridge.submit("coder", it.getJSONObject("element").getJSONObject("props").getString("token"), value); true } ?: false
                else -> gymPacket?.let { gym.buttonIds(it).firstOrNull(::matches) }?.let { bridge.gym(it); true }
                    ?: root?.takeIf { gymPacket?.optString("screen") == "chat" }?.let { findNode(it) { n -> matches(n.optString("key")) } }
                        ?.let { bridge.activate("coder", view, it.optString("key")); true } ?: false
            }
            if (done) gymScript(steps.drop(1), 0) else gymScript(steps, attempt + 1)
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
        // The theme Rust resolved changed: redraw in it.
        if (Palette.apply(bridge.packet?.objectOrNull("appearance"))) restyle()
        // A chat asked to connect a computer: Account > Computers.
        if (bridge.computersRequested != computersShown) {
            computersShown = bridge.computersRequested
            select(AppTab.ACCOUNT)
            open(AccountRoute.COMPUTERS)
        }
        // The chat's attach control asked for a photo.
        if (bridge.imagePicks != imagePicksShown) {
            imagePicksShown = bridge.imagePicks
            if (bridge.attachmentsEnabled) pickImage.launch(androidx.activity.result.PickVisualMediaRequest(
                ActivityResultContracts.PickVisualMedia.ImageOnly))
        }
        // A key that can answer chat was added: ask once whether to use it for everything.
        if (bridge.askMineRequests != askMineShown) {
            askMineShown = bridge.askMineRequests
            account.askMine()
        }
        // An offer under a chat reply opened another screen.
        if (bridge.screenRequests != screensShown) {
            screensShown = bridge.screenRequests
            when (bridge.screenRequested) {
                "wallet" -> select(AppTab.WALLET)
                "keys" -> { select(AppTab.ACCOUNT); open(AccountRoute.IDENTITY) }
                "playtest" -> if (Preview.on) { select(AppTab.ACCOUNT); open(AccountRoute.PLAYTEST) }
                "report" -> report()
                // See the board: the Verse tab, at the Gym's EVALS board.
                "verse_gym" -> if (Preview.on) { select(AppTab.VERSE); panels.openEvals() }
                // Train Coder from the Verse or Account: the Chat tab, on
                // the Gym intro.
                "chat" -> select(AppTab.CODER)
            }
        }
        val packet = bridge.packet
        // A computer that comes online connects Everglade's studio.
        if (::studio.isInitialized) studio.sync()
        if (::connect.isInitialized) connect.update(packet?.objectOrNull("connect"))
        val shell = packet?.objectOrNull("shell")
        homeCards.update(shell)
        mount(coderRenderer, coderContent, fixture ?: packet?.objectOrNull("coder"))
        renderGym(packet?.objectOrNull("gym"))
        val gymScreen = packet?.objectOrNull("gym")?.optString("screen") ?: "chat"
        shellBar.update(shell?.takeIf { gymScreen == "chat" })
        if (drawerOpen) drawer.update(shell)
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
            AccountRoute.KEYS -> if (packet?.objectOrNull("provider_keys")?.toString() + bridge.providerKeyError != shownKeys) redrawAccountScreen()
            AccountRoute.IDENTITY, AccountRoute.CHANGELOG, AccountRoute.TRAINER, AccountRoute.PLAYTEST, AccountRoute.REPORTS -> Unit
            null -> Unit
        }
        if (tab == AppTab.WALLET) wallet.update(packet)
        // An agent's payment request shows over any tab until the owner
        // approves or denies it; Rust closes it.
        if (::payments.isInitialized) payments.update(packet)
        if (::walletLink.isInitialized) walletLink.update(packet)
        val gpuTerminal = tab == AppTab.VERSE && world.nativeError == null &&
            world.snapshot?.optJSONObject("computer_hud")?.let {
                it.optBoolean("visible")
            } == true
        terminal.update(packet?.optBoolean("terminal") == true && !gpuTerminal, bridge.terminalView)
        if (tab == AppTab.VERSE) renderVerse()
    }

    /** The Chat tab's own screen: the menu or a first-run step over the chat, and the Gym's sheet. */
    private fun renderGym(gymPacket: JSONObject?) {
        val screen = gymPacket?.optString("screen") ?: "chat"
        val drawn = when (screen) {
            "menu" -> gymPacket?.objectOrNull("menu")
            "first_run" -> gymPacket?.objectOrNull("first_run")
            else -> null
        }
        val encoded = drawn?.let { "$screen:$it" }
        if (encoded != shownGym) {
            shownGym = encoded
            gymContent.removeAllViews()
            if (drawn != null) gymContent.addView(if (screen == "menu") gym.menu(drawn) else gym.firstRun(drawn),
                FrameLayout.LayoutParams(-1, -1))
        }
        if (drawn != null && gymContent.visibility != View.VISIBLE) {
            // The chat's composer sits under the menu: put its keyboard away.
            currentFocus?.let { focus ->
                getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(focus.windowToken, 0)
                focus.clearFocus()
            }
        }
        // The chat under the menu can't take focus, so its composer never
        // raises a keyboard over the menu.
        coderContent.descendantFocusability =
            if (drawn != null) android.view.ViewGroup.FOCUS_BLOCK_DESCENDANTS else android.view.ViewGroup.FOCUS_AFTER_DESCENDANTS
        gymContent.visibility = if (drawn != null) View.VISIBLE else View.GONE
        coderContent.visibility = if (drawn != null) View.GONE else View.VISIBLE
        gym.sheet(gymPacket?.objectOrNull("sheet"))
        bridge.gymShare?.let { text -> bridge.gymShare = null; gym.share(text) }
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

    // The activity is single-task, so a link that arrives while it runs
    // comes here.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleLink(intent)
    }

    /** The desktop app's QR code is https://openagents.com/connect#<code>:
     *  the system camera opens this app with it, and Rust pairs as if it was
     *  scanned. Rust checks the link; the code rides in the fragment. */
    private fun handleLink(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val link = intent.dataString ?: return
        // Handle each link once, not again when the activity is recreated.
        intent.data = null
        bridge.connectLink(link)
    }

    // The theme (#11028)

    /** Whether the phone itself is dark: the night bits of its uiMode. */
    private fun night(configuration: android.content.res.Configuration) =
        configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK ==
            android.content.res.Configuration.UI_MODE_NIGHT_YES

    /** The manifest keeps the activity across uiMode changes; System follows the phone. */
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        if (::bridge.isInitialized) bridge.systemAppearance(night(newConfig))
    }

    private val appearancePrefs get() = getSharedPreferences("appearance", android.content.Context.MODE_PRIVATE)

    /** The `appearance` Rust last sent, kept only to draw the first frame in it. */
    private fun savedAppearance(): JSONObject? =
        appearancePrefs.getString("last", null)?.let { runCatching { JSONObject(it) }.getOrNull() }

    /** The status and navigation bars follow the theme. */
    private fun restyleWindow() {
        window.decorView.setBackgroundColor(Palette.BACKGROUND)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = Palette.LIGHT
            isAppearanceLightNavigationBars = Palette.LIGHT
        }
    }

    /**
     * Redraws in the theme Rust resolved: the window, the tab bar, the chat,
     * the Account tab and its open screen, the Gym's screens, and the
     * Wallet. Other views take the new colors when they are next built.
     */
    private fun restyle() {
        bridge.packet?.objectOrNull("appearance")?.let { appearancePrefs.edit().putString("last", it.toString()).apply() }
        restyleWindow()
        if (!::root.isInitialized) return
        root.setBackgroundColor(Palette.BACKGROUND)
        body.setBackgroundColor(Palette.BACKGROUND)
        shellBar.update(bridge.packet?.objectOrNull("shell"), force = true)
        drawer.build()
        drawer.update(bridge.packet?.objectOrNull("shell"))
        homeCards = HomeCards(this) { id -> bridge.shell("try_card", "id" to id) }
        menus()
        coderRenderer.clear()
        coderContent.removeAllViews()
        cardViews.clear()
        imageViews.clear()
        shownGym = null
        wallet.update(bridge.packet, force = true)
        // Rebuilds the Account page, and renders again.
        open(route)
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
        worldTerminalKeys.clearFocus()
        world.setResumed(false)
        bridge.lifecycle(false)
        super.onPause()
    }

    override fun onDestroy() {
        main.removeCallbacks(tick)
        terminal.stop()
        routeInput?.dispose()
        if (::connect.isInitialized) connect.dispose()
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
