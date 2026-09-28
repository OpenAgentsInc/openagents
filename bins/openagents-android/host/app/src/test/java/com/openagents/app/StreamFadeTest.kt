package com.openagents.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StreamFadeTest {
    private val duration = StreamFade.DURATION_MS
    private val shown = StreamFade.SHOWN

    /** A run of `text` at `x`, `baseline`, advancing 10 points per character. */
    private fun run(text: String, baseline: Float = 20f, x: Float = 16f, style: Int = 0, scroller: Int = -1) =
        FadeRun(scroller, style, x, baseline, text.length * 10f, text, 0, text.length)

    private fun carry(old: List<FadeRun>, fade: RowFade?, new: List<FadeRun>, now: Long,
                      oldRects: List<FadeRect> = emptyList(), rects: List<FadeRect> = emptyList(), length: Long = duration) =
        StreamFade.carry(old, oldRects, fade, new, rects, now, length)

    @Test fun theFadeMatchesTheIosPainter() {
        assertEquals(180L, StreamFade.DURATION_MS)
    }

    @Test fun appendedTextFadesInAndTheShownPrefixDoesNot() {
        val fade = carry(listOf(run("Hello wor")), null, listOf(run("Hello world")), now = 1_000)!!
        assertEquals(listOf(FadeSegment(0f, shown), FadeSegment(90f, 1_000)), fade.runs[0])
        assertEquals(1f, fade.alpha(shown, 1_000))
        assertEquals(0f, fade.alpha(1_000, 1_000))
        assertEquals(0.5f, fade.alpha(1_000, 1_090), 1e-6f)
        assertEquals(1f, fade.alpha(1_000, 1_180))
        assertTrue(fade.active(1_179))
        assertFalse(fade.active(1_180))
    }

    @Test fun aNewLineOrParagraphFadesInWhole() {
        val fade = carry(listOf(run("First line")), null, listOf(run("First line"), run("second", baseline = 40f)), now = 5)!!
        assertNull(fade.runs[0])
        assertEquals(listOf(FadeSegment(0f, 5)), fade.runs[1])
    }

    @Test fun unchangedRowsDoNotFade() {
        val runs = listOf(run("Same"), run("text", baseline = 40f))
        assertNull(carry(runs, null, runs.map { run(it.text, it.baseline) }, now = 10))
    }

    @Test fun textShownInFullNeverFadesAgain() {
        // The first step fades in "world"; after the fade finishes, a second
        // step keeps it shown and fades only the next word.
        val first = carry(listOf(run("Hello")), null, listOf(run("Hello world")), now = 0)!!
        val second = carry(listOf(run("Hello world")), first, listOf(run("Hello world again")), now = 500)!!
        assertEquals(listOf(FadeSegment(0f, shown), FadeSegment(110f, 500)), second.runs[0])
    }

    @Test fun textStillFadingKeepsItsOwnClock() {
        // Updates faster than the fade: each word keeps the time it arrived,
        // so an earlier word never restarts or jumps back.
        val first = carry(listOf(run("Hello")), null, listOf(run("Hello world")), now = 0)!!
        val second = carry(listOf(run("Hello world")), first, listOf(run("Hello world again")), now = 60)!!
        assertEquals(listOf(FadeSegment(0f, shown), FadeSegment(50f, 0), FadeSegment(110f, 60)), second.runs[0])
        val third = carry(listOf(run("Hello world again")), second, listOf(run("Hello world again!")), now = 200)!!
        // "world" finished at 180 and is shown now; "again" is still fading.
        assertEquals(listOf(FadeSegment(0f, shown), FadeSegment(110f, 60), FadeSegment(170f, 200)), third.runs[0])
    }

    @Test fun changedTextAtTheSamePlaceFadesInWhole() {
        val fade = carry(listOf(run("Hello")), null, listOf(run("Help!")), now = 3)!!
        assertEquals(listOf(FadeSegment(0f, 3)), fade.runs[0])
    }

    @Test fun styleScrollerAndPlaceSeparateRuns() {
        val old = listOf(run("code"))
        assertNotNull(carry(old, null, listOf(run("code", style = 1)), now = 1)!!.runs[0])
        assertNotNull(carry(old, null, listOf(run("code", scroller = 0)), now = 1)!!.runs[0])
        assertNotNull(carry(old, null, listOf(run("code", x = 30f)), now = 1)!!.runs[0])
    }

    @Test fun runsCompareTheirOwnSliceOfAParagraph() {
        // Two lines of one paragraph share a text; each compares its slice.
        val before = "Hello brave new"
        val after = "Hello brave new world"
        val old = listOf(FadeRun(-1, 0, 16f, 20f, 120f, before, 0, 12), FadeRun(-1, 0, 16f, 40f, 30f, before, 12, 3))
        val new = listOf(FadeRun(-1, 0, 16f, 20f, 120f, after, 0, 12), FadeRun(-1, 0, 16f, 40f, 90f, after, 12, 9))
        val fade = carry(old, null, new, now = 7)!!
        assertNull(fade.runs[0])
        assertEquals(listOf(FadeSegment(0f, shown), FadeSegment(30f, 7)), fade.runs[1])
    }

    @Test fun aGrowingBoxKeepsItsStateAndANewBoxFadesIn() {
        val code = FadeRect(-1, 16f, 60f, 300f, 0x11223344, null)
        val table = FadeRect(-1, 16f, 200f, 300f, 0x11223344, null)
        val fade = carry(listOf(run("a")), null, listOf(run("a")), now = 9, oldRects = listOf(code), rects = listOf(code, table))!!
        assertEquals(shown, fade.rects[0])
        assertEquals(9L, fade.rects[1])
        // Still fading at the next update: it keeps its start time, then settles.
        val rects = listOf(code, table)
        assertEquals(9L, carry(listOf(run("a")), fade, listOf(run("a")), now = 100, oldRects = rects, rects = rects)!!.rects[1])
        assertNull(carry(listOf(run("a")), fade, listOf(run("a")), now = 189, oldRects = rects, rects = rects))
    }

    @Test fun animationsOffMeansNoFade() {
        assertNull(carry(listOf(run("Hello")), null, listOf(run("Hello world")), now = 0, length = 0))
    }

    @Test fun aScaledDurationStretchesTheFade() {
        val fade = carry(listOf(run("Hello")), null, listOf(run("Hello world")), now = 0, length = duration * 5)!!
        assertEquals(0.2f, fade.alpha(0, 180), 1e-6f)
        assertTrue(fade.active(899))
    }

    @Test fun longRepliesCompareInLinearTime() {
        // 20,000 lines, the last one growing: every earlier line stays shown.
        val text = (0 until 20_000).joinToString("") { "line $it. " }
        var offset = 0
        val old = ArrayList<FadeRun>(); val new = ArrayList<FadeRun>()
        for (i in 0 until 20_000) {
            val length = "line $i. ".length
            old.add(FadeRun(-1, 0, 16f, 20f * i, length * 10f, text, offset, length))
            new.add(FadeRun(-1, 0, 16f, 20f * i, length * 10f, text, offset, length))
            offset += length
        }
        new.add(run("tail", baseline = 20f * 20_000))
        val started = System.nanoTime()
        val fade = carry(old, null, new, now = 1)!!
        val elapsed = (System.nanoTime() - started) / 1e6
        assertEquals(listOf(FadeSegment(0f, 1)), fade.runs.last())
        assertEquals(1, fade.runs.count { it != null })
        assertTrue("took $elapsed ms", elapsed < 500)
    }
}
