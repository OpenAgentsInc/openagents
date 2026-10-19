// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.*

class TranscriptInteractionsTest {
    @Test fun selectionStaysOnItsRowsWhenAFrameAddsEarlierHistoryAndStreamsText() {
        val original = frame(listOf("hello world", "next row"))
        val state = TranscriptSelectionState()
        state.select(TranscriptTextPoint(20uL, 6), TranscriptTextPoint(21uL, 4))
        assertEquals("world\nnext", state.selectedText(original))

        val updated = frame(listOf("older", "hello world!", "next row"), listOf(19uL, 20uL, 21uL))
        state.reconcile(updated)
        assertEquals("world!\nnext", state.selectedText(updated))
        assertEquals(20uL, state.selection?.anchor?.rowKey)
        assertEquals(6 to 12, state.rangeForRow(updated, 20uL))
    }

    @Test fun removingAnAnchoredRowClearsSelection() {
        val state = TranscriptSelectionState()
        state.select(TranscriptTextPoint(20uL, 0), TranscriptTextPoint(21uL, 2))
        state.reconcile(frame(listOf("kept"), listOf(20uL)))
        assertNull(state.selection)
    }

    @Test fun selectionNeverSplitsASurrogatePair() {
        val state = TranscriptSelectionState()
        val current = frame(listOf("A🙂B"))
        state.select(TranscriptTextPoint(20uL, 2), TranscriptTextPoint(20uL, 3))
        state.reconcile(current)
        assertEquals("🙂", state.selectedText(current))
        assertEquals(1, state.selection?.anchor?.utf16Offset)
    }

    @Test fun tapResolutionUsesRustLinkAndWidgetBounds() {
        val row = placement(0, 20uL, 0f)
        val display = RowDisplay(
            row.key, row.version, 200f, 40f, "link", emptyList(), emptyList(),
            listOf(LinkHit(10f, 4f, 40f, 20f, "https://example.test", null)),
            emptyList(), emptyList(), emptyList(), revealAgeMs = null, copyText = "link", revealSlices = emptyList(),
        )
        val current = FixtureFrame(listOf(row), mapOf(0u to display))
        assertEquals(
            TranscriptHitTarget.Link("https://example.test"),
            TranscriptHitTester.hit(current, 20f, 8f, 0f),
        )
        assertNull(TranscriptHitTester.hit(current, 80f, 8f, 0f))

        val widget = Widget(7u, WidgetKind.CopyCode, 10f, 4f, 40f, 20f, null, null)
        val widgetFrame = FixtureFrame(
            listOf(row),
            mapOf(0u to display.copy(links = emptyList(), widgets = listOf(widget))),
        )
        assertEquals(
            TranscriptHitTarget.Widget(row.key, widget.id, widget.kind),
            TranscriptHitTester.hit(widgetFrame, 20f, 8f, 0f),
        )
    }

    private fun frame(text: List<String>, keys: List<ULong> = listOf(20uL, 21uL)): FixtureFrame {
        val rows = text.mapIndexed { index, value -> placement(index, keys[index], index * 20f) }
        val displays = rows.mapIndexed { index, row ->
            index.toUInt() to RowDisplay(
                row.key, row.version, 200f, 20f, text[index], emptyList(), emptyList(),
                emptyList(), emptyList(), emptyList(), emptyList(), revealAgeMs = null, copyText = text[index], revealSlices = emptyList(),
            )
        }.toMap()
        return FixtureFrame(rows, displays)
    }

    private fun placement(index: Int, key: ULong, y: Float) =
        RowPlacement(index.toUInt(), key, 1uL, y, 20f, RowKind.MARKDOWN)

    private class FixtureFrame(
        private val rows: List<RowPlacement>,
        private val displays: Map<UInt, RowDisplay>,
    ) : TranscriptFrame {
        override val revision = 1uL
        override val width = 200f
        override val totalHeight = rows.sumOf { it.height.toDouble() }.toFloat()
        override fun styles() = emptyList<StyleDesc>()
        override fun rowsIn(top: Float, bottom: Float) = rows.filter { it.y < bottom && it.y + it.height > top }
        override fun indexAt(y: Float): UInt? = rows.indexOfFirst { y >= it.y && y < it.y + it.height }
            .takeIf { it >= 0 }?.toUInt()
        override fun placement(index: UInt) = rows.getOrNull(index.toInt())
        override fun indexOf(key: ULong) = rows.indexOfFirst { it.key == key }.takeIf { it >= 0 }?.toUInt()
        override fun display(index: UInt) = displays[index]
    }
}
