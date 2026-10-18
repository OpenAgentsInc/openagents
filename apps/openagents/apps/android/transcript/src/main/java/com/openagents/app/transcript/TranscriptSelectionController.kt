// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

/** Native callbacks never outlive the binding/frame that produced their geometry. */
data class TranscriptSelectionToken(val binding: Long, val frame: Long)
data class TranscriptSelectionBookmark(val conversation: String, val selection: TranscriptTextSelection)

/** Presentation state only; this borrows frames owned by TranscriptCanvas. */
class TranscriptSelectionController(
    boundaries: TranscriptTextBoundaries = TranscriptCharacterBoundaries,
    private val changed: (TranscriptTextSelection?) -> Unit = {},
) {
    companion object { private val bindings = java.util.concurrent.atomic.AtomicLong(0) }
    val state = TranscriptSelectionState(boundaries)
    private var binding = bindings.incrementAndGet()
    private var serial = 0L
    private var conversation: String? = null
    private var frame: TranscriptFrame? = null
    private var closed = false
    val token get() = TranscriptSelectionToken(binding, serial)
    val currentFrame get() = frame

    /** Call before Canvas takes ownership and closes its previous frame. */
    fun submit(next: TranscriptFrame, conversation: String): Boolean {
        if (closed || conversation.isEmpty() || conversation.length > 512) return false
        if (this.conversation == conversation && frame != null && next.revision <= frame!!.revision) return false
        if (this.conversation != conversation) {
            binding = bindings.incrementAndGet()
            state.clear()
            this.conversation = conversation
        }
        serial++
        frame = next
        state.reconcile(next)
        changed(state.selection)
        return true
    }
    fun accepts(value: TranscriptSelectionToken) = !closed && frame != null && token == value
    fun select(value: TranscriptSelectionToken, anchor: TranscriptTextPoint, focus: TranscriptTextPoint): Boolean {
        if (!accepts(value) || anchor.utf16Offset < 0 || focus.utf16Offset < 0) return false
        val current = frame ?: return false
        val previous = state.selection
        state.select(anchor, focus)
        state.reconcile(current)
        if (state.selection == null || state.selectedText(current) == null) {
            state.clear()
            previous?.let { state.select(it.anchor, it.focus) }
            return false
        }
        changed(state.selection)
        return true
    }
    fun moveHandle(value: TranscriptSelectionToken, anchorHandle: Boolean, point: TranscriptTextPoint): Boolean {
        val selected = state.selection ?: return false
        return if (anchorHandle) select(value, point, selected.focus) else select(value, selected.anchor, point)
    }
    fun selectRow(value: TranscriptSelectionToken, key: ULong): Boolean {
        if (!accepts(value)) return false
        val current = frame ?: return false
        val display = current.indexOf(key)?.let(current::display) ?: return false
        return select(value, TranscriptTextPoint(key, 0), TranscriptTextPoint(key, display.text.length))
    }
    /** Both clipboard and accessibility read this exact range at invocation time. */
    fun selectedText(value: TranscriptSelectionToken): String? =
        if (accepts(value)) frame?.let(state::selectedText) else null
    fun copy(value: TranscriptSelectionToken, clipboard: (String) -> Unit): Boolean {
        val text = selectedText(value)?.takeIf { it.isNotEmpty() } ?: return false
        clipboard(text)
        return true
    }
    fun bookmark(): TranscriptSelectionBookmark? {
        val key = conversation ?: return null
        return state.selection?.let { TranscriptSelectionBookmark(key, it) }
    }
    fun restore(value: TranscriptSelectionToken, bookmark: TranscriptSelectionBookmark): Boolean =
        bookmark.conversation == conversation && select(value, bookmark.selection.anchor, bookmark.selection.focus)
    /** Cancel/rotation retires callbacks; do not save frame handles or selected text. */
    fun cancel() {
        serial++
        state.clear()
        changed(null)
    }
    fun close() {
        if (closed) return
        closed = true
        serial++
        binding = bindings.incrementAndGet()
        state.clear()
        frame = null
        conversation = null
        changed(null)
    }
}

/** C1 must notify this observer before any borrowed frame is closed. */
interface TranscriptInteractionObserver {
    fun beforeFrameReplacement(next: TranscriptFrame)
    fun viewportChanged()
    fun beforeFrameClose()
}
