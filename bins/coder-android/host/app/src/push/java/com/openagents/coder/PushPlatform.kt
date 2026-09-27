package com.openagents.coder

import android.os.Handler
import android.os.Looper
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

/**
 * Firebase Messaging. Gradle compiles this file only when
 * app/google-services.json is present; otherwise src/nopush supplies a stub.
 * Tokens stay in memory and go only to Rust's `push_token`.
 */
object PushPlatform {
    const val AVAILABLE = true
    private val main = Handler(Looper.getMainLooper())
    private var deliver: ((String) -> Unit)? = null
    private var pending: String? = null

    /** Call on the main thread. Fetches the current token; rotations arrive through the service. */
    fun start(deliver: (String) -> Unit, failed: (String) -> Unit) {
        this.deliver = deliver
        pending?.let { pending = null; deliver(it) }
        FirebaseMessaging.getInstance().token
            .addOnSuccessListener { token(it) }
            .addOnFailureListener { failed("Couldn't register for wakes: ${it.message ?: "Firebase Messaging failed."}") }
    }

    fun stop() { deliver = null }

    internal fun token(value: String) = main.post {
        deliver?.invoke(value) ?: run { pending = value }
    }
}

/** Receives token rotations. A wake itself carries only a fixed reconnect constant. */
class CoderMessagingService : FirebaseMessagingService() {
    override fun onNewToken(token: String) { PushPlatform.token(token) }
    override fun onMessageReceived(message: RemoteMessage) {
        // The next launch or foreground reconnects; the message holds nothing to act on.
    }
}
