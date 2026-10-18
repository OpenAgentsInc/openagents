package com.openagents.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.openagents.app.adaptive.*
import com.openagents.app.accessibility.*
import com.openagents.app.composer.*
import com.openagents.app.drawer.*
import com.openagents.app.onboarding.*
import com.openagents.app.sessions.AndroidSessionsHost
import com.openagents.app.settings.*
import com.openagents.app.transcript.*
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.openagents_core.*

/** Activity-owned mounts over the Application's retained Rust account and navigation owner. */
internal class AndroidRootPresentation(
    private val activity: MainActivity,
    private val application: OpenAgentsApplication,
    private val owner: CoreAndroidHost,
) : AutoCloseable {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val reads = Mutex()
    private var closed = false
    private var queued = false
    private var dirty = false
    private var foreground = true
    private val semanticMounts = mutableMapOf<AndroidHostSurface, NativeSemantics>()
    private var snapshot: AndroidHostSnapshot? = null
    private var contentRevision: ULong? = null
    private var mountedIdentity: Pair<ULong, ULong>? = null
    private var accountGeneration: ULong? = null
    private var imeBottom = 0
    private var chat: AndroidChatPresentation? = null
    private var canvas: TranscriptCanvas? = null
    private var layout: TranscriptView? = null
    private var relay: TranscriptFrameRelay? = null
    private var composer: AndroidComposerPresentation? = null
    private var attachments: AndroidComposerAttachmentResults? = null
    private var sessions: AndroidSessionsHost? = null
    private var settings: SettingsHost? = null
    private var recovery: RecoveryHost? = null
    private var settingsActions: AndroidSettingsActions? = null
    private var drawer: DrawerPresenter? = null
    private val notice = TextView(activity)
    private val navigation = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }
    private val drawerView = DrawerView(activity)
    private val content = FrameLayout(activity)
    private val chatContainer = FrameLayout(activity)
    private val routeContainer = FrameLayout(activity)
    private lateinit var panes: AdaptivePaneHost<CoreAndroidHost, CoreAndroidHost, AndroidTranscriptAnchor>
    private val observation: AutoCloseable
    private val root = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }

    init {
        val buttons = LinearLayout(activity)
        fun action(label: String, run: () -> Unit) = Button(activity).apply {
            text = label
            setOnClickListener { guarded(run) }
        }
        buttons.addView(action("Chat") { owner.selectDefaultChat(); panes.showSession() })
        buttons.addView(action("New chat") { owner.navigate(AndroidHostRoute.SESSIONS); panes.showSession() })
        buttons.addView(action("Settings") { owner.navigate(AndroidHostRoute.SETTINGS); panes.showSession() })
        if (owner.account() != null) buttons.addView(action("Your ID") { owner.navigate(AndroidHostRoute.RECOVERY); panes.showSession() })
        navigation.addView(buttons)
        navigation.addView(drawerView, LinearLayout.LayoutParams(-1, 0, 1f))
        content.addView(chatContainer, FrameLayout.LayoutParams(-1, -1))
        content.addView(routeContainer, FrameLayout.LayoutParams(-1, -1))
        panes = AdaptivePaneHost(activity, navigation, content, {
            val density = activity.resources.displayMetrics.density
            AdaptiveWindowMetrics(
                (panesViewWidth() / density).toInt(),
                (panesViewHeight() / density).toInt(),
                activity.resources.configuration.fontScale, (imeBottom / density).toInt(),
            )
        }, {
            AdaptiveRetainedState(snapshot?.selectedChat, null, owner, owner, snapshot?.anchor)
        })
        val toolbar = LinearLayout(activity)
        toolbar.addView(action("Chats") { panes.showNavigation() })
        toolbar.addView(notice, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        root.addView(toolbar)
        root.addView(panes.view, LinearLayout.LayoutParams(-1, 0, 1f))
        observation = application.observeAndroidHost(::refresh)
        ViewCompat.setOnApplyWindowInsetsListener(panes.view) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            imeBottom = keyboard.bottom
            view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            panes.refresh()
            insets
        }
        refresh()
    }
    // These callbacks run only after the pane's constructor has assigned its view.
    private fun panesViewWidth() = panes.view.width.takeIf { it > 0 } ?: activity.resources.displayMetrics.widthPixels
    private fun panesViewHeight() = panes.view.height.takeIf { it > 0 } ?: activity.resources.displayMetrics.heightPixels
    fun view(): View = root

    fun refresh() {
        ui()
        if (closed) return
        dirty = true
        if (!foreground) return
        if (queued) return
        queued = true
        scope.launch {
            try {
                reads.withLock {
                    do {
                        dirty = false
                        val next = withContext(Dispatchers.IO) { owner.snapshot() }
                        if (!closed && !dirty) mount(next)
                    } while (!closed && dirty)
                }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (!closed) showFailure() }
            finally { queued = false }
        }
    }

    private fun mount(next: AndroidHostSnapshot) {
        ui()
        if (accountGeneration != next.generation) {
            closeRoutes()
            closeChat()
            drawer?.detach(); drawer = null
            contentRevision = null
            accountGeneration = next.generation
            bindDrawer(next)
        }
        snapshot = next
        if (contentRevision != next.contentRevision) {
            closeChat()
            mountChat(next)
            contentRevision = next.contentRevision
        } else if (next.route == AndroidHostRoute.CHAT) {
            // Saved-history updates also feed the existing Rust layout, without native message rows.
            owner.refreshTranscript(requireNotNull(layout), next.generation, next.contentRevision)
        }
        val composerFrame = next.composer?.frame()
        composerFrame?.let { composer?.render(AndroidComposerFeature.frame(it)) }
        when (next.route) {
            AndroidHostRoute.CHAT -> {
                closeRoutes()
                routeContainer.visibility = View.GONE
                chatContainer.visibility = View.VISIBLE
            }
            AndroidHostRoute.SESSIONS -> {
                recovery?.close(); recovery = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE
                routeContainer.visibility = View.VISIBLE
                if (sessions == null) next.sessions?.let { core ->
                    sessions = AndroidSessionsHost(routeContainer, core, scope, ::sessionAction, { showFailure() }, { semantics(AndroidHostSurface.SESSIONS) })
                    sessions?.showPicker()
                }
            }
            AndroidHostRoute.RECOVERY -> {
                sessions?.close(); sessions = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE; routeContainer.visibility = View.VISIBLE
                if (recovery == null) next.recovery?.let { core ->
                    recovery = RecoveryHost(routeContainer, activity.window, CoreRecoveryPort(core, ownsCore = false),
                        AndroidDeviceAuthentication(activity, CredentialLauncher(activity::launchDeviceCredential)),
                        RecoveryClipboard(activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager),
                        application::recoveryWork,
                        { guarded { owner.selectDefaultChat(); panes.showSession() } },
                        { semantics(AndroidHostSurface.RECOVERY) },
                    ).also { it.start(android.os.Build.MODEL) }
                }
            }
            AndroidHostRoute.SETTINGS -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                chatContainer.visibility = View.GONE
                routeContainer.visibility = View.VISIBLE
                if (settings == null) mountSettings(next)
            }
        }
        val message = composerFrame?.error ?: next.notice
        notice.text = message.orEmpty()
        notice.visibility = if (message == null) View.GONE else View.VISIBLE
        if (next.client != null && drawer == null) bindDrawer(next)
        drawer?.show()
        panes.refresh()
    }

    private fun mountChat(next: AndroidHostSnapshot) {
        val transcript = TranscriptCanvas(activity, application.transcriptFonts, palette())
        var engine: TranscriptView? = null
        val updates = TranscriptFrameRelay(transcript) { RustTranscriptFrame(requireNotNull(engine).frame()) }
        try {
            val bound = TranscriptView(application.transcriptText, updates)
            engine = bound
            transcript.onViewportChanged = { width, scale -> bound.setViewport(width, scale) }
            var reportedMount = false
            transcript.onPaintStats = { stats ->
                if (!closed && !reportedMount && stats.visibleRows > 0) {
                    reportedMount = true
                    scope.launch {
                        try { owner.transcriptMounted(next.generation, next.contentRevision) }
                        catch (cancelled: CancellationException) { throw cancelled }
                        catch (_: Exception) { /* Opening again retries a refused read marker. */ }
                    }
                }
            }
            owner.attachTranscript(bound, next.generation, next.contentRevision)
            val presentation = next.composer?.let { AndroidComposerFeature.createHost(activity, it) }
            if (presentation != null) {
                val combined = AndroidChatPresentation(activity, transcript, presentation)
                chat = combined
                composer = presentation
                attachments = AndroidComposerAttachmentResults(activity, presentation, activity)
                chatContainer.addView(combined.view(), FrameLayout.LayoutParams(-1, -1))
            } else chatContainer.addView(transcript, FrameLayout.LayoutParams(-1, -1))
            canvas = transcript; layout = bound; relay = updates
            mountedIdentity = next.generation to next.contentRevision
            next.anchor?.let { transcript.restoreViewportAnchor(TranscriptViewportAnchor(it.row, it.inside, it.following)) }
            transcript.submitFrame(RustTranscriptFrame(bound.frame()))
        } catch (error: Exception) {
            updates.close(); engine?.close(); transcript.close()
            throw error
        }
    }

    private fun bindDrawer(next: AndroidHostSnapshot) {
        val client = next.client ?: return
        val history = next.history ?: return
        val core = RustDrawerCore(client, history, {
            snapshot?.let { current -> current.selectedChat?.let { id ->
                when (current.source) {
                    AndroidHostContent.LIVE -> DrawerKey(DrawerOrigin.LIVE, id)
                    AndroidHostContent.SAVED -> DrawerKey(DrawerOrigin.SAVED, id)
                    AndroidHostContent.INDEXED -> DrawerKey(DrawerOrigin.INDEXED, id)
                    AndroidHostContent.CLOUD -> null
                }
            } }
        },
            { session -> guarded { owner.selectSession(session.chatId()); panes.showSession() } },
            { id -> guarded { owner.selectSavedChat(id); panes.showSession() } },
            { detail -> openIndexed(detail.row.key) },
            acknowledgeOnOpen = false,
            openIndexedRoute = ::openIndexed,
        )
        val presenter = DrawerPresenter(core, scope, drawerView::render)
        drawer = presenter
        drawerView.onRequest = presenter::show
        drawerView.onIntent = presenter::send
        presenter.show()
    }

    private fun openIndexed(key: String) {
        scope.launch {
            try { owner.selectIndexedChat(key); panes.showSession(); refresh() }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (!closed) showFailure() }
        }
    }

    private fun semantics(surface: AndroidHostSurface): NativeSemantics {
        semanticMounts.remove(surface)?.close()
        return NativeSemantics(CoreSemanticsPort(owner.accessibilitySurface(surface), ownsCore = false)).also {
            semanticMounts[surface] = it
            it.environment(activity.resources.configuration.fontScale, activity.resources.displayMetrics.density,
                !android.animation.ValueAnimator.areAnimatorsEnabled())
        }
    }

    private fun mountSettings(next: AndroidHostSnapshot) {
        val account = next.account ?: return
        val callback = object : SettingsListener {
            override fun onSettingsChanged() { activity.runOnUiThread { if (!closed) settings?.refresh() } }
        }
        val core = openSettings(account, next.client, SettingsCapabilities(
            appearance = false, wallpaperPicker = false, pairing = false, cloud = next.client != null,
            archive = false, projectSharing = false, notificationSettings = true, shareChooser = true,
        ), BuildConfig.VERSION_NAME, callback)
        val effects = AndroidSettingsActions(activity, 4101, {}, { _, _, _, _ -> }, null, null, null)
        settingsActions = effects
        settings = SettingsHost(activity, routeContainer, RustSettingsPort(core), scope,
            effects::handle, ::showFailure, semantics(AndroidHostSurface.SETTINGS)).also { it.start() }
    }

    private fun sessionAction(action: AndroidSessionAction) {
        guarded {
            when (action) {
                is AndroidSessionAction.Cloud -> { owner.selectCloudChat(action.chat); panes.showSession() }
                is AndroidSessionAction.Session -> { owner.selectSession(action.id); panes.showSession() }
                is AndroidSessionAction.OpenUrl -> activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(action.url)))
                is AndroidSessionAction.Clipboard -> (activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                    .setPrimaryClip(ClipData.newPlainText("OpenAgents", action.text))
                AndroidSessionAction.Updated -> sessions?.showPicker()
                AndroidSessionAction.Sharing -> showFailure()
            }
        }
    }

    fun handleBack(): Boolean {
        ui()
        if (snapshot?.route != AndroidHostRoute.CHAT) { guarded { owner.navigate(AndroidHostRoute.CHAT) }; return true }
        return panes.handleBack()
    }
    fun background() {
        if (closed) return
        foreground = false
        recovery?.background()
        settings?.close(); settings = null
        sessions?.retire()
        semanticMounts.values.forEach { runCatching { it.lifecycle("background") } }
        runCatching { owner.setForeground(false) }
    }
    fun foreground() {
        if (closed) return
        foreground = true
        runCatching { owner.setForeground(true) }
        semanticMounts.values.forEach { runCatching { it.lifecycle("foreground") } }
        recovery?.foreground(); sessions?.showPicker(); refresh()
    }
    fun configurationChanged() {
        if (closed) return
        semanticMounts.values.forEach { semantics -> runCatching {
            semantics.lifecycle("rotation")
            semantics.environment(activity.resources.configuration.fontScale, activity.resources.displayMetrics.density,
                !android.animation.ValueAnimator.areAnimatorsEnabled())
        } }
        canvas?.refreshViewport(); panes.refresh()
        recovery?.refresh(); settings?.refresh(); sessions?.showPicker()
    }
    private fun guarded(action: () -> Unit) {
        if (closed) return
        try { action(); refresh() } catch (_: Exception) { showFailure() }
    }
    private fun showFailure() { notice.text = "Couldn't open this screen. Try again."; notice.visibility = View.VISIBLE }
    private fun closeRoutes() {
        recovery?.close(); recovery = null
        sessions?.close(); sessions = null
        settings?.close(); settings = null
        settingsActions?.close(); settingsActions = null
        routeContainer.removeAllViews()
    }
    private fun closeChat() {
        val current = mountedIdentity
        canvas?.saveViewportAnchor()?.let { anchor ->
            if (current != null) try {
                owner.saveTranscriptAnchor(current.first, current.second,
                    AndroidTranscriptAnchor(anchor.row, anchor.inside, anchor.following))
            } catch (_: Exception) { /* A retired account or conversation cannot accept this bookmark. */ }
        }
        attachments?.close(); attachments = null
        // Stop queued frames before releasing their Rust source or borrowed frame handles.
        relay?.close(); relay = null
        chat?.close()
        if (chat == null) canvas?.close()
        chat = null; canvas = null; composer = null; mountedIdentity = null
        layout?.close(); layout = null
        chatContainer.removeAllViews()
    }
    override fun close() {
        ui(); if (closed) return
        closed = true
        observation.close()
        scope.cancel()
        closeRoutes(); closeChat()
        drawer?.detach(); drawer = null
        drawerView.onRequest = {}; drawerView.onIntent = {}
        ViewCompat.setOnApplyWindowInsetsListener(panes.view, null)
        panes.close()
        semanticMounts.values.forEach { it.close() }; semanticMounts.clear()
    }
    private fun ui() = check(Looper.myLooper() == Looper.getMainLooper())
    private fun palette() = TranscriptPalette { role ->
        val resource = when (role) {
            ColorRole.TEXT_SECONDARY, ColorRole.TEXT_TERTIARY, ColorRole.TEXT_FAINT, ColorRole.TEXT_SOFT -> R.color.oa_secondary
            ColorRole.USER_BUBBLE, ColorRole.CODE_BACKGROUND, ColorRole.INLINE_CODE_BACKGROUND,
            ColorRole.TABLE_HEADER_BACKGROUND, ColorRole.CHIP_BACKGROUND, ColorRole.TOOL_BADGE,
            ColorRole.TOOL_WELL, ColorRole.AGENT_CARD, ColorRole.AGENT_TILE -> R.color.oa_surface
            ColorRole.RULE, ColorRole.CODE_BORDER, ColorRole.TABLE_BORDER, ColorRole.TOOL_RAIL, ColorRole.AGENT_CARD_BORDER -> R.color.oa_hairline
            else -> R.color.oa_text
        }
        activity.getColor(resource)
    }
}
