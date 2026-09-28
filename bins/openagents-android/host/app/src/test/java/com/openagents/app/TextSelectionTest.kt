package com.openagents.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TextSelectionTest {
    /** A run whose UTF-16 units each advance `advance` points, except those listed as zero-width. */
    private fun run(text: Int, start: Int, length: Int, x: Float, baseline: Float, advance: Float = 10f,
                    scroller: Int = -1, zero: Set<Int> = emptySet(), limit: Float? = null,
                    width: Float = length * advance - zero.size * advance): SelectableRun =
        SelectableRun(text, start, length, x, baseline, width, 12f, 4f, scroller, limit) {
            val carets = FloatArray(length + 1)
            for (i in 0 until length) carets[i + 1] = carets[i] + if (i in zero) 0f else advance
            carets
        }

    // One paragraph wrapped onto two lines, a second paragraph, and a code line in a scroller.
    private val texts = listOf("Hello brave new world", "Second", "let x = 1;")
    private val text = SelectableText(texts, listOf(
        run(0, 0, 12, 16f, 20f),   // "Hello brave "
        run(0, 12, 9, 16f, 40f),   // "new world"
        run(1, 0, 6, 16f, 70f),    // "Second"
        null,                      // a run Rust placed that has no usable text
        run(2, 0, 10, 20f, 100f, scroller = 0),
    ))

    @Test fun selectAllSpansTheFirstAndLastUsableRuns() {
        assertTrue(text.hasText)
        assertEquals(TextPosition(0, 0) to TextPosition(4, 10), text.textBounds())
        assertFalse(SelectableText(emptyList(), listOf(null)).hasText)
        assertNull(SelectableText(emptyList(), emptyList()).textBounds())
    }

    @Test fun textJoinsAParagraphsLinesAndBreaksBetweenParagraphs() {
        val (start, end) = text.textBounds()!!
        assertEquals("Hello brave new world\nSecond\nlet x = 1;", text.text(start, end))
        assertEquals("brave new", text.text(TextPosition(0, 6), TextPosition(1, 3)))
        assertEquals("", text.text(TextPosition(1, 3), TextPosition(0, 6)))
    }

    @Test fun cellsOnOneBaselineJoinWithATab() {
        val table = SelectableText(listOf("Name", "Value"), listOf(run(0, 0, 4, 0f, 10f), run(1, 0, 5, 100f, 10f)))
        assertEquals("Name\tValue", table.text(TextPosition(0, 0), TextPosition(1, 5)))
    }

    @Test fun caretsSitAtRustsPositionsWithTheLinesHeight() {
        assertEquals(Caret(16f + 60f, 8f, 24f), text.caret(TextPosition(0, 6)))
        assertEquals(Caret(16f + 90f, 28f, 44f), text.caret(TextPosition(1, 99)))
        assertNull(text.caret(TextPosition(3, 0)))
        assertEquals(0, text.scroller(4))
        assertEquals(-1, text.scroller(3))
    }

    @Test fun caretStopsScaleToRustsMeasuredWidth() {
        val scaled = SelectableText(listOf("abcd"), listOf(run(0, 0, 4, 0f, 10f, width = 20f)))
        assertEquals(15f, scaled.caret(TextPosition(0, 3))!!.x, 0.001f)
        assertEquals(20f, scaled.caret(TextPosition(0, 4))!!.x, 0.001f)
    }

    @Test fun aTruncatedRunsCaretsStopAtItsLimit() {
        val clipped = SelectableText(listOf("abcdefgh"), listOf(run(0, 0, 8, 0f, 10f, limit = 35f)))
        assertEquals(30f, clipped.caret(TextPosition(0, 3))!!.x, 0.001f)
        assertEquals(35f, clipped.caret(TextPosition(0, 8))!!.x, 0.001f)
    }

    @Test fun aPointFindsTheNearestLineThenTheNearestCaret() {
        assertEquals(TextPosition(0, 3), text.position(16f + 31f, 18f, -1))
        assertEquals(TextPosition(1, 9), text.position(500f, 41f, -1))
        assertEquals(TextPosition(2, 0), text.position(0f, 200f, -1))
        // Above the first line, the first line.
        assertEquals(TextPosition(0, 0), text.position(0f, -50f, -1))
        // Only the scroller's runs answer for the scroller.
        assertEquals(TextPosition(4, 5), text.position(70f, 20f, 0))
        assertNull(text.position(0f, 0f, 7))
    }

    @Test fun aCaretNeverLandsInsideAClusterOrSurrogatePair() {
        // "a", then an emoji as a surrogate pair, then "b": the pair's second unit advances nothing.
        val emoji = SelectableText(listOf("a😀b"), listOf(run(0, 0, 4, 0f, 10f, zero = setOf(2))))
        assertEquals(TextPosition(0, 1), emoji.position(10.5f, 10f, -1))
        assertEquals(TextPosition(0, 3), emoji.position(19.9f, 10f, -1))
        assertEquals("😀", emoji.text(TextPosition(0, 1), TextPosition(0, 3)))
    }

    @Test fun highlightsCoverTheSelectionByScroller() {
        val boxes = text.highlights(TextPosition(1, 4), TextPosition(4, 3))
        assertEquals(listOf(TextBox(16f + 40f, 28f, 16f + 90f, 44f), TextBox(16f, 58f, 16f + 60f, 74f)), boxes[-1])
        assertEquals(listOf(TextBox(20f, 88f, 50f, 104f)), boxes[0])
        assertTrue(text.highlights(TextPosition(2, 2), TextPosition(2, 2)).isEmpty())
    }
}
