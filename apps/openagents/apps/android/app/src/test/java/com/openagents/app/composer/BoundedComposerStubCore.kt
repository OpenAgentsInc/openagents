package com.openagents.app.composer

import java.nio.charset.StandardCharsets

/** A bounded fixture core for adapter previews. It performs no session or file work. */
class BoundedComposerStubCore(initial: ComposerFrame) : ComposerCorePort {
    private var frame = initial
    private var text = initial.initialDraft
    val events = ArrayList<String>()
    val displayedViews = ArrayList<Pair<String, Long>>()
    val currentDraft: String get() = text
    var acceptNextSubmit = true

    init {
        require(initial.maxBytes in 1..MAX_TEXT_BYTES)
        require(text.toByteArray(StandardCharsets.UTF_8).size <= initial.maxBytes)
    }

    override fun edit(update: ComposerEditorUpdate): ComposerEditorAck {
        if (update.stamp != frame.stamp ||
            update.text.toByteArray(StandardCharsets.UTF_8).size > frame.maxBytes ||
            update.selectionStartUtf16 !in 0..update.text.length ||
            update.selectionEndUtf16 !in 0..update.text.length ||
            !validMarkedRange(update.markedStartUtf16, update.markedEndUtf16, update.text.length)
        ) return currentAck()

        text = update.text
        frame = frame.copy(stamp = frame.stamp.copy(editorRevision = frame.stamp.editorRevision + 1))
        record("edit:${update.text.length}")
        return ComposerEditorAck(
            stamp = frame.stamp,
            text = text,
            selectionStartUtf16 = update.selectionStartUtf16.coerceIn(0, text.length),
            selectionEndUtf16 = update.selectionEndUtf16.coerceIn(0, text.length),
            markedStartUtf16 = update.markedStartUtf16,
            markedEndUtf16 = update.markedEndUtf16,
        )
    }

    override fun submit(
        stamp: ComposerEditorStamp,
        choiceToken: String?,
        text: String,
        attachments: List<ComposerAttachmentPayload>,
    ): Boolean {
        if (stamp != frame.stamp ||
            choiceToken != null && frame.sendChoices.none { it.token == choiceToken } ||
            text.toByteArray(StandardCharsets.UTF_8).size > frame.maxBytes ||
            attachments.size > MAX_ATTACHMENTS ||
            attachments.any { it.data.isEmpty() || it.data.size > MAX_ATTACHMENT_BYTES }
        ) return false
        if (!acceptNextSubmit) {
            acceptNextSubmit = true
            record("submit_failed")
            return false
        }
        record(if (choiceToken == null) "submit" else "choice:$choiceToken")
        return true
    }

    override fun acknowledge(instance: String, revision: Long) {
        if (validId(instance) && revision > 0 && displayedViews.size < MAX_EVENTS) displayedViews += instance to revision
    }

    override fun changeField(instance: String, revision: Long, node: String, value: String) {
        if (!validId(instance) || !validId(node) || revision <= 0 ||
            value.toByteArray(StandardCharsets.UTF_8).size > MAX_TEXT_BYTES
        ) return
        record("field:$node:${value.toByteArray(StandardCharsets.UTF_8).size}")
    }

    override fun activate(instance: String, revision: Long, node: String) {
        val valid = instance == frame.stamp.instance && revision == frame.stamp.viewRevision &&
            (frame.attachmentActions + frame.mentionSuggestions + frame.attachments.mapNotNull { it.removeAction } +
                listOfNotNull(frame.stopAction)).any { it.node == node && it.enabled }
        if (valid) record("activate:$node")
    }

    private fun currentAck() = ComposerEditorAck(
        stamp = frame.stamp,
        text = text,
        selectionStartUtf16 = text.length,
        selectionEndUtf16 = text.length,
        markedStartUtf16 = null,
        markedEndUtf16 = null,
    )

    private fun validMarkedRange(start: Int?, end: Int?, length: Int): Boolean = when {
        start == null && end == null -> true
        start == null || end == null -> false
        else -> start in 0..length && end in start..length
    }

    private fun validId(value: String) = value.length in 1..96 && value.all { it.code in 0x21..0x7e }

    private fun record(event: String) {
        if (events.size == MAX_EVENTS) events.removeAt(0)
        events += event
    }

    private companion object {
        const val MAX_EVENTS = 32
        const val MAX_TEXT_BYTES = 64 * 1024
        const val MAX_ATTACHMENTS = 10
        const val MAX_ATTACHMENT_BYTES = 24 * 1024 * 1024
    }
}
