// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.Context
import uniffi.openagents_core.CoreAndroidComposer
import uniffi.openagents_core.AndroidComposerFrame
import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.SessionHandle

/** Creates the feature-local ports over the signed-in Rust client and session. */
object AndroidComposerFeature {
    fun createHost(context: Context, owner: CoreAndroidComposer): AndroidComposerPresentation {
        val core = RustComposerCorePort(owner)
        return AndroidComposerPresentation(context, core, SemanticComposerMount(context, core))
    }

    fun frame(value: AndroidComposerFrame): ComposerFrame = ComposerFrame(
        stamp = ComposerEditorStamp(value.instance, value.revision.toLong(), "composer", value.token,
            value.editingLifetime.toString(), value.editorRevision.toLong()),
        initialDraft = value.draft,
        maxBytes = value.maxBytes.toInt(),
        placeholder = value.placeholder,
        enabled = value.enabled,
        busy = value.busy,
        primaryLabel = value.primaryLabel,
        stopAction = value.stopKey?.let { ComposerAction(it, "Stop", true) },
        attachmentActions = value.attachmentKey?.let { listOf(ComposerAction(it, "Attach", true)) }.orEmpty(),
    )

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
