package com.openagents.app

import android.app.Application
import android.os.Handler
import android.os.Looper
import com.openagents.app.platform.AndroidEncryptedFileStore
import com.openagents.app.platform.AndroidPushNotifications
import com.openagents.app.push.AndroidPushFlavor
import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.OwnerNoteView
import uniffi.openagents_core.PushPrefs
import kotlinx.coroutines.launch

private const val MAX_PENDING_OWNER_NOTES = 16
internal interface AndroidHostObservation : AutoCloseable { fun foreground(visible: Boolean) }

class OpenAgentsApplication : Application(), AndroidPushClientProvider {
    internal val presentationWork = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.IO)
    private val stateLock = Any()
    private val mainHandler = Handler(Looper.getMainLooper())
    private val pendingOwnerNotes = ArrayDeque<OwnerNoteView>()

    @Volatile
    private var signedInClient: CoreClient? = null

    @Volatile
    private var signedInPushPrefs = PushPrefs(done = false, input = false, failed = false)

    @Volatile
    private var ownerNotificationHandler: ((OwnerNoteView) -> Unit)? = null
    private lateinit var secretStorage: AndroidEncryptedFileStore
    private val recoveryWorker by lazy {
        java.util.concurrent.ThreadPoolExecutor(1, 1, 0L, java.util.concurrent.TimeUnit.MILLISECONDS,
            java.util.concurrent.ArrayBlockingQueue<Runnable>(32),
            java.util.concurrent.ThreadFactory { task -> Thread(task, "openagents-recovery") },
            java.util.concurrent.ThreadPoolExecutor.AbortPolicy())
    }
    fun recoveryWork(work: () -> Unit) { recoveryWorker.execute { work() } }
    private val voiceResources = GenerationOwnedResource<AndroidActiveVoice>()
    private val activeVoice: AndroidActiveVoice? get() = voiceResources.value
    private var realHost: uniffi.openagents_core.CoreAndroidHost? = null
    private var demoHost: uniffi.openagents_core.CoreAndroidHost? = null
    private var pushGeneration: ULong? = null
    private var pushPermission: Boolean? = null
    private var presentationForeground = false
    private var hostObserver: (() -> Unit)? = null
    private val hostListener = object : uniffi.openagents_core.AndroidHostListener {
        override fun onAndroidHostChanged() { mainHandler.post {
            // Account retirement must stop microphone/service resources even with no Activity mounted.
            val generation = realHost?.account()?.snapshot()?.generation
            if (generation != null) {
                voiceResources.retireFor(generation)
                if (pushGeneration != null && pushGeneration != generation) signedInClient?.let { clearPushClient(it) }
            }
            hostObserver?.invoke()
        } }
    }
    val transcriptFonts by lazy {
        com.openagents.app.transcript.TranscriptFonts(assets,
            com.openagents.app.transcript.TranscriptFonts.geistAssetPaths)
    }
    val transcriptText by lazy {
        uniffi.openagents_core.TextSystem(transcriptFonts.faces,
            com.openagents.app.transcript.AndroidTranscriptMeasurer(transcriptFonts))
    }

    /** Activity mounts borrow these process-owned Rust handles. */
    fun androidHost(demo: Boolean): uniffi.openagents_core.CoreAndroidHost {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (demo) return demoHost ?: uniffi.openagents_core.CoreAndroidHost.demo(hostListener)
            .also { demoHost = it }
        return realHost ?: run {
            val preferences = getSharedPreferences("android-install", MODE_PRIVATE)
            val device = preferences.getString("device-id", null) ?: ("android-" + java.util.UUID.randomUUID()).also {
                check(preferences.edit().putString("device-id", it).commit())
            }
            uniffi.openagents_core.CoreAndroidHost.production(
                java.io.File(filesDir, "core").absolutePath, device, android.os.Build.MODEL,
                BuildConfig.VERSION_NAME, encryptedFileStore(), hostListener,
            ).also { realHost = it; it.start() }
        }
    }

    internal fun startVoice(owner: uniffi.openagents_core.CoreAndroidHost, snapshot: uniffi.openagents_core.AndroidHostSnapshot) {
        check(Looper.myLooper() == Looper.getMainLooper())
        check(activeVoice == null)
        val resources = AndroidActiveVoice(this, owner, snapshot.generation,
            { hostObserver?.invoke() }, { ended -> voiceResources.remove(ended) })
        voiceResources.install(snapshot.generation, resources)
        resources.start(snapshot.contentRevision)
    }

    internal fun voice(snapshot: uniffi.openagents_core.AndroidHostSnapshot): AndroidActiveVoice? {
        check(Looper.myLooper() == Looper.getMainLooper())
        val current = activeVoice ?: return null
        if (current.generation != snapshot.generation || snapshot.activeVoice == null) {
            current.close()
            return null
        }
        return current
    }

    internal fun observeAndroidHost(observer: () -> Unit): AndroidHostObservation {
        check(Looper.myLooper() == Looper.getMainLooper())
        hostObserver = observer
        presentationForeground = true
        return object : AndroidHostObservation {
            override fun foreground(visible: Boolean) { if (hostObserver === observer) presentationForeground = visible }
            override fun close() { if (hostObserver === observer) { hostObserver = null; presentationForeground = false } }
        }
    }
    internal fun synchronizePush(owner: uniffi.openagents_core.CoreAndroidHost, snapshot: uniffi.openagents_core.AndroidHostSnapshot) {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (!AndroidPushFlavor.enabled || snapshot.account == null) return
        val client = snapshot.client
        if (client == null || !snapshot.signedIn) {
            signedInClient?.let { clearPushClient(it) }
            return
        }
        val allowed = AndroidPushNotifications.canPostNotifications(this)
        val changed = pushGeneration != snapshot.generation || pushPermission != allowed
        if (!changed) return
        if (pushGeneration != snapshot.generation) installPushClient(client, owner.defaultPushPrefs())
        pushGeneration = snapshot.generation
        pushPermission = allowed
        if (!allowed) {
            currentClient()?.let { current -> presentationWork.launch { runCatching { current.unregisterPushTarget() } } }
            return
        }
        val generation = snapshot.generation
        AndroidPushFlavor.currentToken { token -> mainHandler.post {
            if (token != null && pushGeneration == generation && AndroidPushNotifications.canPostNotifications(this)) {
                com.openagents.app.platform.AndroidPushCoordinator(this, presentationWork).registerToken(token)
            }
        } }
    }



    override fun onCreate() {
        super.onCreate()
        System.loadLibrary("openagents_mobile")
        AndroidCoreBootstrap.initialize(this)
        secretStorage = AndroidEncryptedFileStore(this)
        if (AndroidPushFlavor.enabled) AndroidPushNotifications.createChannels(this)
    }

    /** The single platform store passed to Rust account and wallet adapters. */
    fun encryptedFileStore(): AndroidEncryptedFileStore {
        AndroidCoreBootstrap.requireInitialized()
        check(::secretStorage.isInitialized)
        return secretStorage
    }

    override fun currentClient(): CoreClient? = signedInClient

    override fun canRegisterPush(): Boolean = AndroidPushFlavor.enabled && signedInClient != null &&
        AndroidPushNotifications.canPostNotifications(this)

    override fun currentPushPrefs(): PushPrefs = synchronized(stateLock) {
        signedInPushPrefs.copy()
    }

    override fun dispatchOwnerNotification(note: OwnerNoteView) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            deliverOwnerNotification(note)
        } else {
            mainHandler.post { deliverOwnerNotification(note) }
        }
    }

    /** Installs the signed-in Rust client and its notification preferences. */
    fun installPushClient(client: CoreClient, prefs: PushPrefs) {
        AndroidCoreBootstrap.requireInitialized()
        synchronized(stateLock) {
            signedInPushPrefs = prefs.copy()
            signedInClient = client
        }
        ownerNotificationHandler = null
        clearPendingOwnerNotesOnMain()
    }

    /** Clears the client only when it is still the installed account client. */
    fun clearPushClient(client: CoreClient) {
        var cleared = false
        synchronized(stateLock) {
            if (signedInClient === client) {
                signedInClient = null
                signedInPushPrefs = PushPrefs(done = false, input = false, failed = false)
                cleared = true
            }
        }
        if (cleared) {
            pushGeneration = null; pushPermission = null
            AndroidOwnerNotifications.clear(this)
            ownerNotificationHandler = null
            clearPendingOwnerNotesOnMain()
        }
    }

    /** Sets the app's foreground and notification-tap router. */
    fun setOwnerNotificationHandler(handler: ((OwnerNoteView) -> Unit)?) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            installOwnerNotificationHandler(handler)
        } else {
            mainHandler.post { installOwnerNotificationHandler(handler) }
        }
    }

    private fun installOwnerNotificationHandler(handler: ((OwnerNoteView) -> Unit)?) {
        ownerNotificationHandler = handler
        if (handler == null) return

        while (pendingOwnerNotes.isNotEmpty()) {
            handler(pendingOwnerNotes.removeFirst())
        }
    }

    private fun deliverOwnerNotification(note: OwnerNoteView) {
        if (presentationForeground) {
            ownerNotificationHandler?.invoke(note)
            return // Rust's chat state still carries attention; foreground never opens a chat automatically.
        }
        if (AndroidOwnerNotifications.show(this, note)) return
        if (pendingOwnerNotes.size == MAX_PENDING_OWNER_NOTES) pendingOwnerNotes.removeFirst()
        pendingOwnerNotes.addLast(note)
    }

    private fun clearPendingOwnerNotesOnMain() {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            pendingOwnerNotes.clear()
        } else {
            mainHandler.post { pendingOwnerNotes.clear() }
        }
    }
}
