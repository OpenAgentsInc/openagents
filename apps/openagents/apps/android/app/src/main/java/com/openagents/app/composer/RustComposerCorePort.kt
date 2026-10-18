// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.os.SystemClock
import org.json.JSONArray
import org.json.JSONObject
import uniffi.openagents_core.OutgoingAttachment
import uniffi.openagents_core.SessionHandle

/** Calls the existing mobile Rust session and Rust Native editor contracts. */
class RustComposerCorePort(
    private val session: SessionHandle,
) : ComposerCorePort {
    override fun mountEditor(
        stamp: ComposerEditorStamp,
        maxBytes: Int,
        draft: String,
    ): ComposerEditorAck {
        val request = JSONObject()
            .put("op", "mount")
            .put("token", stamp.inputToken)
            .put("max_bytes", maxBytes)
            .put("draft", draft)
        return callEditor(stamp, request)
    }

    override fun edit(update: ComposerEditorUpdate): ComposerEditorAck {
        val stamp = update.stamp
        val mark = if (update.markedStartUtf16 != null && update.markedEndUtf16 != null) {
            JSONArray().put(update.markedStartUtf16).put(update.markedEndUtf16)
        } else JSONObject.NULL
        val request = JSONObject()
            .put("op", "apply")
            .put(
                "stamp",
                JSONObject()
                    .put("token", stamp.inputToken)
                    .put("lifetime", stamp.editingLifetime.toLongOrNull() ?: -1L)
                    .put("revision", stamp.editorRevision),
            )
            .put(
                "change",
                JSONObject()
                    .put("op", "sync")
                    .put("text", update.text)
                    .put("selection", JSONArray().put(update.selectionStartUtf16).put(update.selectionEndUtf16))
                    .put("marked", mark)
                    .put("at_ms", SystemClock.elapsedRealtime()),
            )
        return callEditor(stamp, request)
    }

    override fun submit(
        stamp: ComposerEditorStamp,
        choiceToken: String?,
        text: String,
        attachments: List<ComposerAttachmentPayload>,
    ): Boolean {
        session.submitNativeComposer(
            stamp.instance,
            stamp.viewRevision,
            stamp.inputToken,
            stamp.editingLifetime.toLongOrNull() ?: -1L,
            stamp.editorRevision,
            choiceToken,
            text,
            attachments.map { attachment ->
                OutgoingAttachment(attachment.name, attachment.mimeType, attachment.data.copyOf())
            },
        )
        return true
    }

    override fun activate(instance: String, revision: Long, node: String) {
        session.activateNative(instance, revision, node, null, emptyList())
    }

    override fun activateAttachment(instance: String, revision: Long, node: String): Boolean {
        session.activateNative(instance, revision, node, null, emptyList())
        return true
    }

    override fun changeField(instance: String, revision: Long, node: String, value: String) {
        session.changeNativeField(instance, revision, node, value)
    }

    override fun acknowledge(instance: String, revision: Long) {
        session.acknowledgeNativeView(instance, revision)
    }

    fun prepareNativeView(viewJson: String): String =
        session.prepareNativeView(viewJson.toByteArray(Charsets.UTF_8)).toString(Charsets.UTF_8)

    private fun callEditor(stamp: ComposerEditorStamp, request: JSONObject): ComposerEditorAck {
        val response = session.composerEditor(
            stamp.instance,
            stamp.viewRevision,
            request.toString().toByteArray(Charsets.UTF_8),
        )
        val root = JSONObject(response.toString(Charsets.UTF_8))
        val state = root.optJSONObject("state")
            ?: throw IllegalStateException("Rust could not restore the composer draft.")
        val editorStamp = state.getJSONObject("stamp")
        val selection = state.getJSONArray("selection")
        val marked = state.optJSONArray("marked")
        val acknowledgedStamp = stamp.copy(
            inputToken = editorStamp.getString("token"),
            editingLifetime = editorStamp.getLong("lifetime").toString(),
            editorRevision = editorStamp.getLong("revision"),
        )
        return ComposerEditorAck(
            stamp = acknowledgedStamp,
            text = state.getString("text"),
            selectionStartUtf16 = selection.getInt(0),
            selectionEndUtf16 = selection.getInt(1),
            markedStartUtf16 = marked?.getInt(0),
            markedEndUtf16 = marked?.getInt(1),
        )
    }
}
