// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.*

/** Authored synthetic frame states, not captured engine/device output. */
class TranscriptSelectionLifecycleTest {
    private class Frame(
        override val revision: ULong,
        private val keys: List<ULong>,
        private val texts: List<String>,
        private val overlays: Map<Int, List<Widget>> = emptyMap(),
        private val fades: Map<Int, List<Fade>> = emptyMap(),
    ) : TranscriptFrame {
        var reads = 0
        private val indices = keys.mapIndexed { index, key -> key to index }.toMap()
        override val width = 320f
        override val totalHeight = keys.size * 24f
        override fun styles() = emptyList<StyleDesc>()
        override fun indexAt(y: Float) = (y / 24f).toInt().takeIf { y >= 0 && it in keys.indices }?.toUInt()
        override fun indexOf(key: ULong) = indices[key]?.toUInt()
        override fun placement(index: UInt) = keys.getOrNull(index.toInt())?.let {
            RowPlacement(index, it, revision, index.toFloat() * 24f, 24f, RowKind.MARKDOWN)
        }
        override fun rowsIn(top: Float, bottom: Float) = keys.indices
            .filter { it * 24f < bottom && (it + 1) * 24f > top }.mapNotNull { placement(it.toUInt()) }
        override fun display(index: UInt): RowDisplay? {
            reads++
            val at = index.toInt(); val key = keys.getOrNull(at) ?: return null
            return RowDisplay(key, revision, width, 24f, texts[at], emptyList(), emptyList(), emptyList(),
                emptyList(), overlays[at].orEmpty(), fades[at].orEmpty(), texts[at])
        }
    }
    private fun frame(revision: ULong, vararg text: String) = Frame(revision, text.indices.map { it.toULong() + 20uL }, text.toList())

    @Test fun reversedDragKeepsItsAnchorAndProducesTheSameClipboardAndAccessibleText() {
        val controller = TranscriptSelectionController()
        val frame = frame(1uL, "hello world", "next row")
        controller.submit(frame, "synthetic-account/room")
        val token = controller.token
        assertTrue(controller.select(token, TranscriptTextPoint(21uL, 4), TranscriptTextPoint(20uL, 6)))
        val copied = mutableListOf<String>()
        assertTrue(controller.copy(token) { copied += it })
        assertEquals(listOf("world\nnext"), copied)
        assertEquals(controller.selectedText(token), copied.single())
        assertEquals(TranscriptTextPoint(21uL, 4), controller.state.selection!!.anchor)
        assertTrue(controller.moveHandle(token, false, TranscriptTextPoint(20uL, 0)))
        assertEquals("hello world\nnext", controller.selectedText(token))
    }
    @Test fun combiningAndSurrogateBoundariesNeverYieldPartialVisibleCharacters() {
        val controller = TranscriptSelectionController()
        controller.submit(frame(1uL, "Ae\u0301🙂B"), "room")
        assertTrue(controller.select(controller.token, TranscriptTextPoint(20uL, 2), TranscriptTextPoint(20uL, 4)))
        assertEquals("e\u0301🙂", controller.selectedText(controller.token))
        assertEquals(1, controller.state.selection!!.anchor.utf16Offset)
        assertEquals(5, controller.state.selection!!.focus.utf16Offset)
    }
    @Test fun staleActionModeCopyAndLateHandleCallbacksCannotUseAReplacementFrame() {
        val controller = TranscriptSelectionController()
        controller.submit(frame(1uL, "first"), "room")
        val old = controller.token
        controller.selectRow(old, 20uL)
        controller.submit(frame(2uL, "first appended"), "room")
        val copied = mutableListOf<String>()
        assertFalse(controller.copy(old) { copied += it })
        assertFalse(controller.moveHandle(old, false, TranscriptTextPoint(20uL, 14)))
        assertTrue(copied.isEmpty())
        assertEquals("first", controller.selectedText(controller.token))
    }
    @Test fun insertionRemovalAndStreamingNeverMoveTheUsersFixedEndpoints() {
        val controller = TranscriptSelectionController()
        controller.submit(frame(1uL, "hello world", "next"), "room")
        controller.select(controller.token, TranscriptTextPoint(20uL, 6), TranscriptTextPoint(20uL, 11))
        controller.submit(Frame(2uL, listOf(19uL, 20uL, 21uL), listOf("older", "hello world!!", "next")), "room")
        assertEquals("world", controller.selectedText(controller.token))
        assertEquals(TranscriptTextPoint(20uL, 6), controller.state.selection!!.anchor)
        controller.submit(Frame(3uL, listOf(21uL), listOf("next")), "room")
        assertNull(controller.state.selection)
    }
    @Test fun rotationCancelAndReopenNeedFreshTokensAndTheSameAccountConversation() {
        val controller = TranscriptSelectionController()
        controller.submit(frame(1uL, "hello"), "account-a/room")
        controller.selectRow(controller.token, 20uL)
        val saved = controller.bookmark()!!
        val beforeRotation = controller.token
        controller.cancel()
        assertFalse(controller.copy(beforeRotation) { fail("stale copy") })
        controller.close()
        assertFalse(controller.submit(frame(2uL, "hello"), "account-a/room"))
        val reopened = TranscriptSelectionController()
        reopened.submit(frame(1uL, "hello"), "account-a/room")
        assertFalse(reopened.select(beforeRotation, saved.selection.anchor, saved.selection.focus))
        assertTrue(reopened.restore(reopened.token, saved))
        assertEquals("hello", reopened.selectedText(reopened.token))
        reopened.submit(frame(2uL, "other"), "account-b/room")
        assertFalse(reopened.restore(reopened.token, saved))
        assertNull(reopened.state.selection)
    }
    @Test fun longTranscriptCopiesOnlyAnchoredRowsAndRejectsOversizedSelections() {
        val keys = (0 until 20000).map { it.toULong() }
        val texts = keys.map { "synthetic row $it" }
        val frame = Frame(1uL, keys, texts)
        val controller = TranscriptSelectionController()
        controller.submit(frame, "room")
        frame.reads = 0
        assertTrue(controller.select(controller.token, TranscriptTextPoint(18000uL, 0), TranscriptTextPoint(18001uL, 4)))
        assertEquals("synthetic row 18000\nsynt", controller.selectedText(controller.token))
        assertTrue(frame.reads < 20)
        assertFalse(controller.select(controller.token, TranscriptTextPoint(0uL, 0), TranscriptTextPoint(19999uL, 4)))
        val huge = this.frame(2uL, "x".repeat(65537))
        controller.submit(huge, "room")
        assertFalse(controller.selectRow(controller.token, 20uL))
    }
    @Test fun stopErrorRetryFadeAndAttachmentFixturesDoNotChangeSelectionOrInventActions() {
        val controller = TranscriptSelectionController()
        val fade = Fade(100f, 0f, 40f, 24f, FadeEdge.TRAILING, null)
        for ((index, status) in listOf("Streaming", "Stopped", "Failed", "Retrying").withIndex()) {
            val widget = Widget(7u, WidgetKind.ToolStatus(running = index == 0 || index == 3, failed = index == 2), 0f, 0f, 10f, 10f, null, null)
            val frame = Frame((index + 1).toULong(), listOf(20uL, 21uL), listOf("selected text", status),
                mapOf(1 to listOf(widget)), if (index == 0) mapOf(0 to listOf(fade)) else emptyMap())
            controller.submit(frame, "room")
            if (index == 0) controller.select(controller.token, TranscriptTextPoint(20uL, 0), TranscriptTextPoint(20uL, 8))
            assertEquals("selected", controller.selectedText(controller.token))
            assertEquals(if (index == 0) listOf(fade) else emptyList<Fade>(), controller.currentFrame!!.display(0u)!!.fades)
            assertNull(TranscriptHitTester.hit(frame, 1f, 25f, 0f)) // ToolStatus is display-only, never retry/stop authority.
        }
        val attachment = Widget(8u, WidgetKind.Image("synthetic-artifact"), 0f, 0f, 20f, 20f, null, null)
        val frame = Frame(5uL, listOf(20uL, 22uL), listOf("selected text", "Picture"), mapOf(1 to listOf(attachment)))
        controller.submit(frame, "room")
        assertEquals("selected", controller.selectedText(controller.token))
        assertEquals(TranscriptHitTarget.Widget(22uL, 8u, attachment.kind), TranscriptHitTester.hit(frame, 1f, 25f, 0f))
    }
}
