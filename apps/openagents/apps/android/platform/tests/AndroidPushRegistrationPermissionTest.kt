package com.openagents.app.platform

import android.test.AndroidTestCase
import com.openagents.app.AndroidPushClientProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.OwnerNoteView
import uniffi.openagents_core.PushPrefs

/** Denial must return before scheduling or touching a Rust client, including FCM token refresh. */
class AndroidPushRegistrationPermissionTest : AndroidTestCase() {
    fun testDeniedPermissionDoesNotQueueClientLookupOrRegistration() {
        var clientReads = 0
        var preferenceReads = 0
        val provider = object : AndroidPushClientProvider {
            override fun canRegisterPush() = false
            override fun currentClient(): CoreClient? { clientReads++; return null }
            override fun currentPushPrefs(): PushPrefs { preferenceReads++; error("No preferences should be read") }
            override fun dispatchOwnerNotification(note: OwnerNoteView) = error("No note should be delivered")
        }
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            AndroidPushCoordinator(provider, scope).registerToken("synthetic-fcm-token")
            assertEquals(0, clientReads)
            assertEquals(0, preferenceReads)
        } finally { scope.cancel() }
    }
}
