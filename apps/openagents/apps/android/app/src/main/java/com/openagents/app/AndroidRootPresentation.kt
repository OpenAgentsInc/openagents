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
import com.openagents.app.platform.AndroidVoiceOrbSurface
import com.openagents.app.storage.AndroidRetiredSparkSeed
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
    private var transcriptSurfaces: AndroidTranscriptSurfaces? = null
    private var labActions: AndroidLabActionsMount? = null
    private var labActionsInstance: String? = null
    private val labActionsContainer = FrameLayout(activity)
    private var layout: TranscriptView? = null
    private var relay: TranscriptFrameRelay? = null
    private var notificationOpen: Job? = null
    private var mediaLoad: Job? = null
    private var mediaViewer: TranscriptMediaViewer? = null
    private var mediaRequest = 0L
    private var selection: TranscriptSelectionHost? = null
    private var selectionCallbacks = false
    private var transcriptNavigation: TranscriptNavigation? = null
    private var composer: AndroidComposerPresentation? = null
    private var attachments: AndroidComposerAttachmentResults? = null
    private var sessions: AndroidSessionsHost? = null
    private var settings: SettingsHost? = null
    private var sharing: AndroidSharingMount? = null
    private var recovery: RecoveryHost? = null
    private var wallet: RecoveryHost? = null
    private var voiceSettings: AndroidVoiceSettingsMount? = null
    private lateinit var voiceSettingsButton: Button
    private lateinit var walletButton: Button
    private var linkReview: AndroidLinkReviewMount? = null
    private var microphoneRequest: AutoCloseable? = null
    private var voiceCapsule: SemanticViewAdapter? = null
    private var voiceScreen: SemanticViewAdapter? = null
    private var voiceOrb: AndroidVoiceOrbSurface? = null
    private var voiceResources: AndroidActiveVoice? = null
    private var voiceCall: VoiceCall? = null
    private lateinit var voiceButton: Button
    private val voiceCapsuleContainer = FrameLayout(activity)
    private var settingsActions: AndroidSettingsActions? = null
    private var drawer: DrawerPresenter? = null
    private var ownerNoteObservation: AutoCloseable? = null
    private val ownerNote = TextView(activity)
    private val ownerNoteOpen = Button(activity).apply { text = "Open chat" }
    private val ownerNoteBanner = LinearLayout(activity).apply {
        orientation = LinearLayout.HORIZONTAL
        visibility = View.GONE
        addView(ownerNote, LinearLayout.LayoutParams(0, -2, 1f))
        addView(ownerNoteOpen)
    }
    private val notice = TextView(activity)
    private val navigation = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }
    private val drawerView = DrawerView(activity)
    private val content = FrameLayout(activity)
    private val chatContainer = FrameLayout(activity)
    private val routeContainer = FrameLayout(activity)
    private lateinit var panes: AdaptivePaneHost<CoreAndroidHost, CoreAndroidHost, AndroidTranscriptAnchor>
    private val observation: AndroidHostObservation
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
        walletButton = action("Wallet") { owner.navigate(AndroidHostRoute.WALLET); panes.showSession() }
        buttons.addView(walletButton)
        voiceButton = action("Call") { requestVoice() }
        buttons.addView(voiceButton)
        voiceSettingsButton = action("Voice") { owner.navigate(AndroidHostRoute.VOICE_SETTINGS); panes.showSession() }
        buttons.addView(voiceSettingsButton)
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
        root.addView(ownerNoteBanner)
        root.addView(labActionsContainer)
        root.addView(voiceCapsuleContainer)
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
            closeOwnerNote()
            closeRoutes()
            closeChat()
            drawer?.detach(); drawer = null
            contentRevision = null
            accountGeneration = next.generation
            bindDrawer(next)
        }
        snapshot = next
        application.synchronizePush(owner, next)
        if (foreground && ownerNoteObservation == null && next.signedIn) {
            val generation = next.generation
            ownerNoteObservation = application.setOwnerNotificationHandler { note ->
                if (!closed && foreground && owner.account()?.snapshot()?.generation == generation) {
                    ownerNote.text = "${note.title}\n${note.body}"
                    ownerNoteBanner.visibility = View.VISIBLE
                    ownerNoteOpen.setOnClickListener {
                        if (!closed && foreground && owner.account()?.snapshot()?.generation == generation) {
                            ownerNoteBanner.visibility = View.GONE
                            offerNotificationTap(note.deepLink, note.requestId)
                        }
                    }
                    refresh()
                }
            }
        }
        resolveNotification(next)
        walletButton.visibility = if (next.account != null && next.signedIn) View.VISIBLE else View.GONE
        voiceSettingsButton.visibility = if (next.account != null && next.client != null) View.VISIBLE else View.GONE
        mountVoice(next)
        if (contentRevision != next.contentRevision) {
            closeChat()
            mountChat(next)
            contentRevision = next.contentRevision
        } else if (next.route == AndroidHostRoute.CHAT) {
            // Saved-history updates also feed the existing Rust layout, without native message rows.
            owner.refreshTranscript(requireNotNull(layout), next.generation, next.contentRevision)
        }
        if (next.route != AndroidHostRoute.CHAT) { closeMedia(); transcriptSurfaces?.retire() }
        if (next.route != AndroidHostRoute.SHARING) { sharing?.close(); sharing = null }
        if (next.route != AndroidHostRoute.VOICE_SETTINGS) { voiceSettings?.close(); voiceSettings = null }
        if (next.route != AndroidHostRoute.WALLET) { wallet?.close(); wallet = null }
        if (next.route != AndroidHostRoute.LINK_REVIEW) { linkReview?.close(); linkReview = null }
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
            AndroidHostRoute.SHARING -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE; routeContainer.visibility = View.VISIBLE
                if (sharing == null) {
                    routeContainer.removeAllViews()
                    sharing = AndroidSharingMount(activity, routeContainer, owner.sharing(), application.presentationWork,
                        semantics(AndroidHostSurface.SHARING), ::showFailure)
                } else sharing?.render()
            }
            AndroidHostRoute.VOICE_SETTINGS -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE; routeContainer.visibility = View.VISIBLE
                if (voiceSettings == null) {
                    routeContainer.removeAllViews()
                    voiceSettings = AndroidVoiceSettingsMount(activity, routeContainer, owner, owner.voiceSettings(),
                        application.presentationWork, semantics(AndroidHostSurface.VOICE_AGENT),
                        semantics(AndroidHostSurface.VOICE_USAGE), ::showFailure)
                } else voiceSettings?.render()
            }
            AndroidHostRoute.WALLET -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE; routeContainer.visibility = View.VISIBLE
                if (wallet == null) {
                    val core = owner.wallet(AndroidRetiredSparkSeed(activity.applicationContext))
                    wallet = RecoveryHost(routeContainer, activity.window,
                        AndroidWalletRecoveryPort(core, application::recoveryWork, { wallet?.refresh() }),
                        AndroidDeviceAuthentication(activity, CredentialLauncher(activity::launchDeviceCredential)),
                        RecoveryClipboard(activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager, label = "Wallet"),
                        application::recoveryWork, {}, { semantics(AndroidHostSurface.WALLET) },
                    ).also { it.start("") }
                } else wallet?.refresh()
            }
            AndroidHostRoute.LINK_REVIEW -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                settings?.close(); settings = null
                settingsActions?.close(); settingsActions = null
                chatContainer.visibility = View.GONE; routeContainer.visibility = View.VISIBLE
                next.linkReview?.let { core ->
                    if (linkReview?.id != core.id()) {
                        linkReview?.close()
                        linkReview = AndroidLinkReviewMount(activity, routeContainer, core, application.presentationWork,
                            semantics(AndroidHostSurface.LINK_REVIEW),
                            { guarded { owner.dismissLinkReview(core) } }, ::showFailure).also { it.start() }
                    } else linkReview?.render()
                }
            }
            AndroidHostRoute.VOICE -> {
                closeRoutes()
                chatContainer.visibility = View.GONE
                routeContainer.visibility = View.VISIBLE
                voiceCall?.let { call -> voiceScreen?.mount(routeContainer, call.screenView().toByteArray(Charsets.UTF_8)) }
            }
            AndroidHostRoute.SETTINGS -> {
                recovery?.close(); recovery = null
                sessions?.close(); sessions = null
                chatContainer.visibility = View.GONE
                routeContainer.visibility = View.VISIBLE
                if (settings == null) mountSettings(next)
            }
        }
        mountLabActions(next)
        val message = composerFrame?.error ?: next.notice
        notice.text = message.orEmpty()
        notice.visibility = if (message == null) View.GONE else View.VISIBLE
        if (next.client != null && drawer == null) bindDrawer(next)
        drawer?.show()
        panes.refresh()
    }

    private fun requestVoice() {
        val requested = snapshot ?: return
        if (requested.activeVoice != null) { owner.navigate(AndroidHostRoute.VOICE); return }
        if (!requested.canStartVoice) return
        microphoneRequest?.close()
        microphoneRequest = activity.requestVoiceMicrophone { granted ->
            if (!closed && foreground && granted) guarded { application.startVoice(owner, requested); activity.requestCallNotifications() }
        }
    }

    private fun mountVoice(next: AndroidHostSnapshot) {
        val active = application.voice(next)
        val call = active?.call
        voiceButton.visibility = if (next.canStartVoice || call != null) View.VISIBLE else View.GONE
        if (call == null) { closeVoiceViews(); return }
        if (voiceResources !== active) {
            closeVoiceViews()
            voiceCall = call; voiceResources = active
            val orb = AndroidVoiceOrbSurface(activity)
            voiceOrb = orb
            fun adapter(surface: AndroidHostSurface, extensions: Map<String, SemanticViewAdapter.Extension> = emptyMap()) = SemanticViewAdapter(
                activity,
                activate = { instance, revision, key -> guarded {
                    if (call.activateView(instance, revision.toULong(), key) == VoiceUiAction.OPEN_CALL) {
                        owner.navigate(AndroidHostRoute.VOICE); panes.showSession()
                    }
                } },
                changeField = { _, _, _, _ -> }, acknowledge = { _, _ -> },
                extensions = extensions, semantics = semantics(surface),
            )
            voiceCapsule = adapter(AndroidHostSurface.VOICE_CAPSULE)
            voiceScreen = adapter(AndroidHostSurface.VOICE_SCREEN, mapOf("voice-orb" to orb))
        }
        voiceCapsule?.mount(voiceCapsuleContainer, call.capsuleView().toByteArray(Charsets.UTF_8))
        active.state?.let { voiceOrb?.update(it) }
    }

    private fun closeVoiceViews() {
        voiceCapsule?.dispose(); voiceCapsule = null
        voiceScreen?.dispose(); voiceScreen = null
        voiceOrb = null; voiceCall = null; voiceResources = null
        voiceCapsuleContainer.removeAllViews()
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
                if (!closed && foreground && snapshot?.route == AndroidHostRoute.CHAT) transcriptSurfaces?.refresh()
                if (!closed && !reportedMount && stats.visibleRows > 0) {
                    reportedMount = true
                    scope.launch {
                        try { owner.transcriptMounted(next.generation, next.contentRevision) }
                        catch (cancelled: CancellationException) { throw cancelled }
                        catch (_: Exception) { /* Opening again retries a refused read marker. */ }
                    }
                }
            }
            val surfaces = AndroidTranscriptSurfaces(activity, transcript, bound, owner,
                next.generation, next.contentRevision, next.history, scope) { url ->
                if (!closed && foreground) activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
            }
            transcriptSurfaces = surfaces
            val selectionView = mountSelection(transcript, bound, next, surfaces.view)
            owner.attachTranscript(bound, next.generation, next.contentRevision)
            val presentation = next.composer?.let { AndroidComposerFeature.createHost(activity, it) }
            if (presentation != null) {
                val combined = AndroidChatPresentation(activity, transcript, presentation, selectionView)
                chat = combined
                composer = presentation
                attachments = AndroidComposerAttachmentResults(activity, presentation, activity)
                chatContainer.addView(combined.view(), FrameLayout.LayoutParams(-1, -1))
            } else chatContainer.addView(selectionView, FrameLayout.LayoutParams(-1, -1))
            canvas = transcript; layout = bound; relay = updates
            mountedIdentity = next.generation to next.contentRevision
            next.anchor?.let { transcript.restoreViewportAnchor(TranscriptViewportAnchor(it.row, it.inside, it.following)) }
            transcript.submitFrame(RustTranscriptFrame(bound.frame()))
        } catch (error: Exception) {
            transcriptSurfaces?.close(); transcriptSurfaces = null
            selectionCallbacks = false
            selection?.close(); selection = null
            transcriptNavigation?.close(); transcriptNavigation = null
            updates.close(); engine?.close(); transcript.close()
            throw error
        }
    }

    private fun mountSelection(transcript: TranscriptCanvas, engine: TranscriptView, next: AndroidHostSnapshot, content: View): TranscriptSelectionHost {
        val conversation = "${next.generation}/${next.contentRevision}"
        var restored = next.selection == null
        var replacingFrame = false
        selectionCallbacks = true
        val host = TranscriptSelectionHost(activity, content, transcript::interactionSnapshot,
            AndroidTranscriptSelectionGeometry(application.transcriptFonts),
            object : TranscriptSelectionCallbacks {
                override fun onSelection(token: TranscriptSelectionToken, range: TranscriptTextSelection?) {
                    if (!selectionCallbacks || replacingFrame) return
                    restored = true
                    val frame = transcript.interactionSnapshot()?.frame ?: return
                    runCatching { owner.saveTextSelection(engine, next.generation, next.contentRevision, frame.revision,
                        range?.let { AndroidHostSelection(AndroidHostTextPoint(it.anchor.rowKey, it.anchor.utf16Offset.toUInt()),
                            AndroidHostTextPoint(it.focus.rowKey, it.focus.utf16Offset.toUInt())) }) }
                }
                override fun onHit(token: TranscriptSelectionToken, target: TranscriptHitTarget) {
                    if (!selectionCallbacks || replacingFrame) return
                    val current = transcript.interactionSnapshot() ?: return
                    try {
                        when (target) {
                            is TranscriptHitTarget.Link -> {
                                val url = owner.transcriptLink(engine, next.generation, next.contentRevision,
                                    current.frame.revision, current.top, current.height, target.url)
                                activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                            }
                            is TranscriptHitTarget.Widget -> when (val effect = owner.transcriptWidget(engine,
                                next.generation, next.contentRevision, current.frame.revision, target.rowKey, target.widgetId)) {
                                AndroidTranscriptEffect.Handled -> Unit
                                is AndroidTranscriptEffect.OpenPage -> activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(effect.url)))
                                is AndroidTranscriptEffect.Copy -> (activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                                    .setPrimaryClip(ClipData.newPlainText("OpenAgents", effect.text))
                                is AndroidTranscriptEffect.Media -> openMedia(next, current.frame.revision,
                                    TranscriptHitTarget.Widget(effect.row, effect.widget, effect.kind))
                                AndroidTranscriptEffect.Unavailable -> showFailure()
                            }
                        }
                    } catch (_: Exception) { showFailure() }
                }
                override fun onSystemTextAction(intent: Intent) { activity.startActivity(intent) }
            }, 0x553B82F6, activity.getColor(R.color.oa_accent))
        selection = host
        val pager = next.account?.let { account ->
            TranscriptNavigation(CoreTranscriptNavigationPort(CoreTranscriptAccessibility(account, engine))) { revision, row, _ ->
                if (transcript.interactionSnapshot()?.frame?.revision == revision) {
                    transcript.restoreViewportAnchor(TranscriptViewportAnchor(row, 0f, false))
                }
            }
        }
        transcriptNavigation = pager
        if (pager != null) host.setVirtualActionDelegate(object : TranscriptVirtualActionDelegate {
            override fun actions(revision: ULong, anchorRow: ULong) = try { pager.actions(revision, anchorRow) } catch (_: Exception) { emptyList() }
            override fun perform(actionId: Int, revision: ULong, anchorRow: ULong) = try { pager.perform(actionId, revision, anchorRow) } catch (_: Exception) { false }
            override fun retire() = pager.retire()
        })
        val observer = host.observer(conversation)
        transcript.interactionObserver = object : TranscriptInteractionObserver {
            override fun beforeFrameReplacement(frame: TranscriptFrame) {
                transcriptSurfaces?.retire()
                replacingFrame = true
                observer.beforeFrameReplacement(frame)
            }
            override fun viewportChanged() {
                if (foreground && snapshot?.route == AndroidHostRoute.CHAT) transcriptSurfaces?.refresh()
                observer.viewportChanged()
                replacingFrame = false
                if (!restored) next.selection?.let { saved ->
                    restored = host.restore(TranscriptSelectionBookmark(conversation, TranscriptTextSelection(
                        TranscriptTextPoint(saved.anchor.row, saved.anchor.utf16.toInt()),
                        TranscriptTextPoint(saved.focus.row, saved.focus.utf16.toInt()))))
                }
                if (restored && selectionCallbacks) {
                    val frame = transcript.interactionSnapshot()?.frame
                    if (frame != null) runCatching {
                        owner.saveTextSelection(engine, next.generation, next.contentRevision, frame.revision,
                            host.bookmark()?.selection?.let { range -> AndroidHostSelection(
                                AndroidHostTextPoint(range.anchor.rowKey, range.anchor.utf16Offset.toUInt()),
                                AndroidHostTextPoint(range.focus.rowKey, range.focus.utf16Offset.toUInt())) })
                    }
                }
            }
            override fun beforeFrameClose() { transcriptSurfaces?.retire(); selectionCallbacks = false; observer.beforeFrameClose() }
        }
        return host
    }

    private fun openMedia(next: AndroidHostSnapshot, frameRevision: ULong, target: TranscriptHitTarget.Widget) {
        val history = next.history
        val engine = layout ?: return showFailure()
        closeMedia()
        val request = mediaRequest
        mediaLoad = scope.launch {
            var bytes: ByteArray? = null
            try {
                withContext(Dispatchers.IO) {
                    bytes = if (target.kind is WidgetKind.Image) owner.transcriptImage(engine,
                        next.generation, next.contentRevision, frameRevision, target.rowKey, target.widgetId)
                    else history?.let { TranscriptMediaLoader(it).image(target) }
                }
                val payload = bytes ?: return@launch showFailure()
                if (closed || !foreground || request != mediaRequest || snapshot?.route != AndroidHostRoute.CHAT ||
                    mountedIdentity != (next.generation to next.contentRevision) ||
                    canvas?.interactionSnapshot()?.frame?.revision != frameRevision) return@launch
                val viewer = TranscriptMediaViewer(activity, payload)
                bytes = null // The disposable viewer now owns these bytes, including failed show cleanup.
                mediaViewer = viewer
                if (!viewer.show()) { viewer.close(); mediaViewer = null; showFailure() }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (!closed && request == mediaRequest) showFailure() }
            finally { bytes?.fill(0) }
        }
    }
    private fun closeMedia() {
        mediaRequest++
        mediaLoad?.cancel(); mediaLoad = null
        mediaViewer?.close(); mediaViewer = null
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
            archive = false, projectSharing = true, notificationSettings = true, shareChooser = true,
        ), BuildConfig.VERSION_NAME, callback)
        val effects = AndroidSettingsActions(activity, 4101, {}, { _, _, _, _ -> }, null, null,
            { guarded { owner.navigate(AndroidHostRoute.SHARING); panes.showSession() } })
        settingsActions = effects
        settings = SettingsHost(activity, routeContainer, RustSettingsPort(core), scope,
            effects::handle, ::showFailure, semantics(AndroidHostSurface.SETTINGS)).also { it.start() }
    }

    private fun sessionAction(action: AndroidSessionAction) {
        guarded {
            when (action) {
                is AndroidSessionAction.Cloud -> { owner.selectCloudChat(action.chat); panes.showSession() }
                is AndroidSessionAction.Session -> { owner.selectSession(action.id); panes.showSession(); activity.requestCallNotifications() }
                is AndroidSessionAction.OpenUrl -> activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(action.url)))
                is AndroidSessionAction.Clipboard -> (activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                    .setPrimaryClip(ClipData.newPlainText("OpenAgents", action.text))
                AndroidSessionAction.Updated -> sessions?.showPicker()
                AndroidSessionAction.Sharing -> { owner.navigate(AndroidHostRoute.SHARING); panes.showSession() }
            }
        }
    }

    fun offerNotificationTap(link: String, request: String) {
        if (!closed) guarded { owner.offerNotificationTap(link, request); panes.showSession() }
    }
    private fun resolveNotification(next: AndroidHostSnapshot) {
        if (!next.pendingNotification || next.client == null || notificationOpen?.isActive == true) return
        notificationOpen = scope.launch {
            try {
                do {
                    val result = owner.resolveNotificationTap()
                    val retry = result.retryAfterMs ?: break
                    delay(retry.toLong())
                } while (!closed && foreground)
                if (!closed && foreground) refresh()
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (!closed && snapshot?.pendingNotification == true) showFailure() }
            finally { notificationOpen = null; if (!closed && foreground) refresh() }
        }
    }

    fun offerAppLink(raw: String) {
        if (closed) return
        guarded { owner.offerAppLink(raw); panes.showSession() }
    }

    fun handleBack(): Boolean {
        ui()
        if (snapshot?.route != AndroidHostRoute.CHAT) { guarded { owner.navigate(AndroidHostRoute.CHAT) }; return true }
        return panes.handleBack()
    }
    fun background() {
        if (closed) return
        foreground = false
        observation.foreground(false)
        closeOwnerNote()
        transcriptSurfaces?.retire()
        closeLabActions()
        notificationOpen?.cancel(); notificationOpen = null
        closeMedia()
        microphoneRequest?.close(); microphoneRequest = null
        snapshot?.linkReview?.let { runCatching { owner.dismissLinkReview(it) } }
        linkReview?.close(); linkReview = null
        sharing?.close(); sharing = null
        voiceSettings?.close(); voiceSettings = null
        wallet?.background()
        recovery?.background()
        settings?.close(); settings = null
        sessions?.retire()
        semanticMounts.values.forEach { runCatching { it.lifecycle("background") } }
        runCatching { owner.setForeground(false) }
    }
    fun foreground() {
        if (closed) return
        foreground = true
        observation.foreground(true)
        if (activity.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            snapshot?.let { application.voice(it)?.close() }
        }
        runCatching { owner.setForeground(true) }
        semanticMounts.values.forEach { runCatching { it.lifecycle("foreground") } }
        wallet?.foreground(); recovery?.foreground(); sessions?.showPicker(); refresh()
        canvas?.invalidate()
    }
    fun configurationChanged() {
        if (closed) return
        semanticMounts.values.forEach { semantics -> runCatching {
            semantics.lifecycle("rotation")
            semantics.environment(activity.resources.configuration.fontScale, activity.resources.displayMetrics.density,
                !android.animation.ValueAnimator.areAnimatorsEnabled())
        } }
        canvas?.refreshViewport(); panes.refresh()
        wallet?.refresh(); recovery?.refresh(); settings?.refresh(); sessions?.showPicker()
    }
    private fun guarded(action: () -> Unit) {
        if (closed) return
        try { action(); refresh() } catch (_: Exception) { showFailure() }
    }
    private fun showFailure() { notice.text = "Couldn't open this screen. Try again."; notice.visibility = View.VISIBLE }
    private fun closeRoutes() {
        sharing?.close(); sharing = null
        voiceSettings?.close(); voiceSettings = null
        wallet?.close(); wallet = null
        linkReview?.close(); linkReview = null
        recovery?.close(); recovery = null
        sessions?.close(); sessions = null
        settings?.close(); settings = null
        settingsActions?.close(); settingsActions = null
        routeContainer.removeAllViews()
    }
    private fun closeChat() {
        closeLabActions()
        transcriptSurfaces?.close(); transcriptSurfaces = null
        closeMedia()
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
        selectionCallbacks = false
        selection?.close(); selection = null
        transcriptNavigation?.close(); transcriptNavigation = null
        chat?.close()
        if (chat == null) canvas?.close()
        chat = null; canvas = null; composer = null; mountedIdentity = null
        layout?.close(); layout = null
        chatContainer.removeAllViews()
    }
    private fun mountLabActions(next: AndroidHostSnapshot) {
        val core = next.labActions
        if (!foreground || next.route != AndroidHostRoute.CHAT || core == null) {
            closeLabActions(); return
        }
        var frame = core.frame()
        if (frame.closed) { closeLabActions(); return }
        if (frame.needsPlaybackPosition) {
            core.playbackPosition(transcriptSurfaces?.playbackSeconds(frame.card))
            frame = core.frame()
        }
        if (labActionsInstance != frame.instance) {
            closeLabActions()
            labActions = AndroidLabActionsMount(activity, labActionsContainer, core,
                application.presentationWork, ::showFailure)
            labActionsInstance = frame.instance
        }
        labActions?.render()
    }
    private fun closeLabActions() {
        labActions?.close(); labActions = null
        labActionsInstance = null
    }
    private fun closeOwnerNote() {
        ownerNoteObservation?.close(); ownerNoteObservation = null
        ownerNoteBanner.visibility = View.GONE
        ownerNote.text = ""
        ownerNoteOpen.setOnClickListener(null)
    }
    override fun close() {
        ui(); if (closed) return
        closed = true
        observation.close()
        closeOwnerNote()
        scope.cancel()
        microphoneRequest?.close(); microphoneRequest = null
        closeVoiceViews()
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
