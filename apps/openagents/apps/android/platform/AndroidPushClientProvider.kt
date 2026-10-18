// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app

import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.OwnerNoteView
import uniffi.openagents_core.PushPrefs

/** The app-owned signed-in client and notification handoff. */
interface AndroidPushClientProvider {
    fun currentClient(): CoreClient?

    fun currentPushPrefs(): PushPrefs

    /** Called on the main thread; the app applies foreground suppression and tap routing. */
    fun dispatchOwnerNotification(note: OwnerNoteView)
}
