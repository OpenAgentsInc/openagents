// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.push

import android.util.Log
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import com.openagents.app.AndroidPushClientProvider
import com.openagents.app.platform.AndroidPushCoordinator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/** Firebase transport adapter. Rust owns registration and notification state. */
class OpenAgentsFirebaseMessagingService : FirebaseMessagingService() {
    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onNewToken(token: String) {
        pushCoordinator()?.registerToken(token)
    }

    override fun onMessageReceived(message: RemoteMessage) {
        pushCoordinator()?.receive(message.data)
    }

    private fun pushCoordinator(): AndroidPushCoordinator? {
        val provider = application as? AndroidPushClientProvider
        if (provider == null) {
            Log.w(TAG, "The app does not provide a signed-in push client")
            return null
        }
        return AndroidPushCoordinator(provider, serviceScope)
    }

    private companion object {
        const val TAG = "OpenAgentsPushService"
    }
}
