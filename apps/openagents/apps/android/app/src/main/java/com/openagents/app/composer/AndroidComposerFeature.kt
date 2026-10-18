// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.Context
import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.SessionHandle

/** Creates the feature-local ports over the signed-in Rust client and session. */
object AndroidComposerFeature {
    fun create(
        context: Context,
        client: CoreClient,
        session: SessionHandle,
        deviceId: String,
        chatId: String,
        spaceId: String? = null,
    ): AndroidComposerPresentation {
        val core = RustComposerCorePort(session)
        return AndroidComposerPresentation(
            context = context,
            core = core,
            rustNativeMount = SemanticComposerMount(context, core),
            mentionPort = RustComposerMentionPort(client, deviceId, chatId, spaceId),
        )
    }
}
