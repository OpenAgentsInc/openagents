// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.RowKind
import uniffi.openagents_core.RowPlacement

class TranscriptViewportTest {
    @Test fun historyInsertionPreservesTheCapturedRowAndOffset() {
        val old = RecordedGeometry.frame("initial")
        val next = RecordedGeometry.frame("history_prepend")
        val state = TranscriptViewport()
        state.resize(120f); state.submit(old); state.userScrollTo(220f, false)
        val before = old.placement(old.indexAt(state.offset)!!)!!
        val inside = state.offset - before.y
        state.submit(next)
        val after = next.placement(next.indexAt(state.offset)!!)!!
        assertEquals(before.key, after.key)
        assertEquals(inside, state.offset - after.y, .001f)
        assertEquals(257f, state.offset, .001f)
    }

    @Test fun streamingFollowsOnlyWhenTheUserHasNotScrolledAway() {
        val initial = RecordedGeometry.frame("initial")
        val streaming = RecordedGeometry.frame("stream_growth")
        val state = TranscriptViewport()
        state.resize(120f); state.submit(initial)
        assertEquals(786f, state.offset, .001f)
        state.submit(streaming)
        assertEquals(936f, state.offset, .001f)
        state.userScrollTo(220f, true)
        state.submit(initial)
        assertFalse(state.following)
        assertEquals(220f, state.offset, .001f)
        state.followTail()
        assertTrue(state.following)
        assertEquals(state.maxOffset, state.offset, 0f)
    }

    @Test fun aDragStartingAtTheTailDoesNotRelatchDuringStreaming() {
        val state = TranscriptViewport()
        state.resize(120f); state.submit(RecordedGeometry.frame("initial"))
        state.userScrollTo(state.offset, true)
        state.submit(RecordedGeometry.frame("stream_growth"))
        assertFalse(state.following)
        assertEquals(786f, state.offset, .001f)
    }

    @Test fun fontAndWidthChangesUseTheSameRustAnchor() {
        val initial = RecordedGeometry.frame("initial")
        for (name in listOf("font_scale", "narrow")) {
            val next = RecordedGeometry.frame(name)
            val state = TranscriptViewport()
            state.resize(120f); state.submit(initial); state.userScrollTo(220f, false)
            val before = initial.placement(initial.indexAt(state.offset)!!)!!
            val inside = state.offset - before.y
            state.submit(next)
            val after = next.placement(next.indexOf(before.key)!!)!!
            assertEquals(after.y + minOf(inside, after.height), state.offset, .001f)
        }
    }

    @Test fun cacheReusesHistoryRowsButNotAnotherWidthOrStyleTable() {
        val initial = RecordedGeometry.frame("initial")
        val state = TranscriptViewport()
        state.resize(120f); state.submit(initial)
        val first = state.display(initial.rows[5])
        val prepended = RecordedGeometry.frame("history_prepend")
        state.submit(prepended)
        assertSame(first, state.display(prepended.placement(prepended.indexOf(initial.rows[5].key)!!)!!))
        val narrow = RecordedGeometry.frame("narrow")
        state.submit(narrow)
        assertNotSame(first, state.display(narrow.rows[5]))
        val styled = FixtureFrame(narrow.width, narrow.totalHeight, narrow.rows, narrow.styles().map { it.copy(size = 22f) })
        val old = state.display(narrow.rows[5])
        state.submit(styled)
        assertNotSame(old, state.display(styled.rows[5]))
    }

    @Test fun fastScrollingKeepsTheCacheBoundedAndQueriesOnlyVisibleRows() {
        // Authored stress stub, not a captured Rust runtime result.
        val rows = (0 until 10_000).map { RowPlacement(it.toUInt(), it.toULong(), 1UL, it * 20f, 20f, RowKind.MARKDOWN) }
        val frame = FixtureFrame(390f, 200_000f, rows)
        val state = TranscriptViewport(cacheLimit = 32)
        state.resize(600f); state.submit(frame)
        for (offset in 0..190_000 step 1000) {
            state.userScrollTo(offset.toFloat(), true)
            val visible = state.visibleRows()
            assertEquals(30, visible.size)
            visible.forEach(state::display)
            assertTrue(state.cachedRows <= 32)
        }
        val calls = frame.displayCalls
        state.visibleRows().forEach(state::display)
        assertEquals(calls, frame.displayCalls)
    }

    @Test fun emptyAndShortFramesClampWithoutLosingFollow() {
        val state = TranscriptViewport()
        state.resize(600f); state.submit(FixtureFrame(390f, 0f, emptyList()))
        assertTrue(state.visibleRows().isEmpty())
        assertEquals(0f, state.offset, 0f)
        state.submit(RecordedGeometry.frame("initial")); state.resize(2000f)
        assertEquals(0f, state.offset, 0f)
    }

    @Test fun scalarAdvancesDoNotSplitSurrogatesOrReassignCombiningMarks() {
        assertEquals(listOf(10f, 0f, 20f), scalarAdvances("e\u0301🙂", floatArrayOf(10f, 0f, 20f, 0f)))
        val family = "👨‍👩‍👧‍👦"
        val widths = FloatArray(family.length).also { it[0] = 48f }
        val scalars = scalarAdvances(family, widths)
        assertEquals(family.codePointCount(0, family.length), scalars.size)
        assertEquals(48f, scalars.first(), 0f)
        assertTrue(scalars.drop(1).all { it == 0f })
        assertTrue(scalarAdvances("", floatArrayOf()).isEmpty())
    }

    @Test fun recordedRunsUseUtf16SlicesAndUnchangedRustBaselines() {
        assertEquals(6, recordedText.size)
        for (row in recordedText) for (run in row.runs) {
            val part = row.text.substring(run.start, run.start + run.length)
            assertTrue(part.isNotEmpty())
            assertFalse(part.first().isLowSurrogate())
            assertTrue(run.x >= 0 && run.baseline > 0 && run.baseline <= row.height)
        }
    }
}
