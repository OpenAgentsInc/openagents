// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.os.Handler
import android.os.Looper
import android.util.Log
import com.openagents.app.AndroidPushClientProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.openagents_core.PushWakeKind

internal enum class AndroidPushWake(private val wireValue: String) {
    RECONNECT("reconnect"),
    ATTENTION("attention");

    companion object {
        fun parse(data: Map<String, String>): AndroidPushWake? {
            if (data.size != 1 || data.keys.singleOrNull() != WAKE_KEY) return null
            return values().singleOrNull { it.wireValue == data[WAKE_KEY] }
        }

        private const val WAKE_KEY = "wake"
    }
}

internal fun <T : Any> currentPushClient(expected: T, current: T?): T? =
    expected.takeIf { current === expected }

internal inline fun <T : Any> deliverToCurrentPushClient(
    expected: T,
    current: T?,
    action: (T) -> Unit,
): Boolean {
    val client = currentPushClient(expected, current) ?: return false
    action(client)
    return true
}

internal inline fun <T : Any> registerPushTokenForCurrentClient(
    token: String,
    expected: T,
    current: T?,
    canRegister: Boolean,
    prefs: () -> uniffi.openagents_core.PushPrefs,
    register: (T, String, uniffi.openagents_core.PushPrefs) -> Unit,
): Boolean {
    if (token.isBlank() || !canRegister) return false
    val client = currentPushClient(expected, current) ?: return false
    register(client, token, prefs())
    return true
}

internal fun AndroidPushWake.toRustKind(): PushWakeKind =
    when (this) {
        AndroidPushWake.RECONNECT -> PushWakeKind.RECONNECT
        AndroidPushWake.ATTENTION -> PushWakeKind.ATTENTION
    }

/** Routes the gateway's data-only wake message to the signed-in Rust client. */
class AndroidPushCoordinator(
    private val provider: AndroidPushClientProvider,
    private val scope: CoroutineScope,
) {
    private val mainHandler = Handler(Looper.getMainLooper())

    fun receive(data: Map<String, String>) {
        val wake = AndroidPushWake.parse(data) ?: return
        withCurrentClient { client ->
            scope.launch(Dispatchers.IO) {
                try {
                    val current = currentPushClient(client, provider.currentClient()) ?: return@launch
                    val note = current.handlePushWake(wake.toRustKind()) ?: return@launch
                    mainHandler.post {
                        deliverToCurrentPushClient(client, provider.currentClient()) {
                            provider.dispatchOwnerNotification(note)
                        }
                    }
                } catch (error: Exception) {
                    Log.w(TAG, "Could not process a push wake", error)
                }
            }
        }
    }

    fun registerToken(token: String) {
        if (token.isBlank() || !provider.canRegisterPush()) return
        withCurrentClient { client ->
            registerPushTokenForCurrentClient(
                token = token,
                expected = client,
                current = provider.currentClient(),
                canRegister = provider.canRegisterPush(),
                prefs = provider::currentPushPrefs,
            ) { current, currentToken, prefs ->
                scope.launch(Dispatchers.IO) {
                    try {
                        current.registerPushTarget(currentToken, "production", prefs)
                    } catch (error: Exception) {
                        Log.w(TAG, "Could not register the push token", error)
                    }
                }
            }
        }
    }

    /** Call before the app clears its signed-in client. */
    suspend fun unregisterCurrentTarget() {
        val client = withContext(Dispatchers.Main.immediate) { provider.currentClient() } ?: return
        try {
            withContext(Dispatchers.IO) {
                client.unregisterPushTarget()
            }
        } catch (error: Exception) {
            Log.w(TAG, "Could not unregister the push target", error)
        }
    }

    private fun withCurrentClient(action: (uniffi.openagents_core.CoreClient) -> Unit) {
        fun retry(attemptsLeft: Int) {
            mainHandler.post {
                val client = provider.currentClient()
                if (client != null) {
                    action(client)
                } else if (attemptsLeft > 0) {
                    mainHandler.postDelayed({ retry(attemptsLeft - 1) }, CLIENT_RETRY_MS)
                } else {
                    Log.w(TAG, "No signed-in client became available for push")
                }
            }
        }
        retry(CLIENT_RETRIES)
    }

    private companion object {
        const val TAG = "AndroidPushCoordinator"
        const val CLIENT_RETRIES = 40
        const val CLIENT_RETRY_MS = 250L
    }
}
