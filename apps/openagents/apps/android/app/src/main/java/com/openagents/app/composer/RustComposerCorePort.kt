// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.os.SystemClock
import org.json.JSONArray
import org.json.JSONObject
import uniffi.openagents_core.NativeActionResult
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
        return callEditor(stamp, request.toString())
    }

    override fun edit(update: ComposerEditorUpdate): ComposerEditorAck {
        val stamp = update.stamp
        val mark = if (update.markedStartUtf16 != null && update.markedEndUtf16 != null) {
            JSONArray().put(update.markedStartUtf16).put(update.markedEndUtf16)
        } else JSONObject.NULL
        val request = """
            {"op":"apply","stamp":{"token":${JSONObject.quote(stamp.inputToken)},"lifetime":${lifetime(stamp.editingLifetime)},"revision":${unsigned(stamp.editorRevision, "editor revision")}},"change":{"op":"sync","text":${JSONObject.quote(update.text)},"selection":[${update.selectionStartUtf16},${update.selectionEndUtf16}],"marked":$mark,"at_ms":${SystemClock.elapsedRealtime().coerceAtLeast(0)}}}
        """.trimIndent()
        return callEditor(stamp, request)
    }

    override fun submit(
        stamp: ComposerEditorStamp,
        choiceToken: String?,
        text: String,
        attachments: List<ComposerAttachmentPayload>,
    ): Boolean {
        val result = session.submitNativeComposer(
            stamp.instance,
            unsigned(stamp.viewRevision, "view revision"),
            stamp.inputToken,
            lifetime(stamp.editingLifetime),
            unsigned(stamp.editorRevision, "editor revision"),
            choiceToken,
            text,
            attachments.map { attachment ->
                OutgoingAttachment(attachment.name, attachment.mimeType, attachment.data.copyOf())
            },
        )
        return when (result) {
            is NativeActionResult.SendStarted,
            is NativeActionResult.SendSteered,
            is NativeActionResult.SendQueued -> true
            else -> false
        }
    }

    override fun activate(instance: String, revision: Long, node: String) {
        session.activateNative(instance, unsigned(revision, "view revision"), node, null, emptyList())
    }

    override fun activateAttachment(instance: String, revision: Long, node: String): Boolean {
        return session.activateNative(
            instance,
            unsigned(revision, "view revision"),
            node,
            null,
            emptyList(),
        ) == NativeActionResult.AttachmentPickerRequested
    }

    override fun changeField(instance: String, revision: Long, node: String, value: String) {
        session.changeNativeField(instance, unsigned(revision, "view revision"), node, value)
    }

    override fun acknowledge(instance: String, revision: Long) {
        session.acknowledgeNativeView(instance, unsigned(revision, "view revision"))
    }

    fun prepareNativeView(viewJson: String): String =
        session.prepareNativeView(viewJson.toByteArray(Charsets.UTF_8)).toString(Charsets.UTF_8)

    private fun callEditor(stamp: ComposerEditorStamp, request: String): ComposerEditorAck {
        val response = session.composerEditor(
            stamp.instance,
            unsigned(stamp.viewRevision, "view revision"),
            request.toByteArray(Charsets.UTF_8),
        )
        val root = JSONObject(response.toString(Charsets.UTF_8))
        val state = root.optJSONObject("state")
            ?: throw IllegalStateException("Rust could not restore the composer draft.")
        val editorStamp = state.getJSONObject("stamp")
        val selection = state.getJSONArray("selection")
        val marked = state.optJSONArray("marked")
        val acknowledgedStamp = stamp.copy(
            inputToken = editorStamp.getString("token"),
            editingLifetime = editorStamp.get("lifetime").toString().also(::lifetime),
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

    private fun lifetime(value: String): ULong =
        value.toULongOrNull() ?: throw IllegalArgumentException("Invalid Rust editor lifetime")

    private fun unsigned(value: Long, field: String): ULong {
        require(value >= 0) { "Invalid $field" }
        return value.toULong()
    }
}
