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

private const val MAX_PENDING_OWNER_NOTES = 16

class OpenAgentsApplication : Application(), AndroidPushClientProvider {
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

    override fun onCreate() {
        super.onCreate()
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
        val handler = ownerNotificationHandler
        if (handler != null) {
            handler(note)
            return
        }

        if (pendingOwnerNotes.size == MAX_PENDING_OWNER_NOTES) {
            pendingOwnerNotes.removeFirst()
        }
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
