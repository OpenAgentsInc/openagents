// The streaming fade. When a row keeps its key and gets new content, such as
// a streamed reply, its new text fades in over the iOS painter's 0.18 s while
// the text already shown stays at full strength. iOS crossfades the row's
// layer (NativeTranscriptView.fade in
// bins/coder-ios/host/App/NativeTranscriptPainter.swift); here the painter
// knows which runs and which parts of runs are new, so it fades only those.
// This file decides that from Rust's runs; it has no Android dependencies,
// so its unit tests run on the JVM.
package com.openagents.app

/** A text run as the fade compares it: where Rust placed it, and its UTF-16 text. */
class FadeRun(
    val scroller: Int, val style: Int, val x: Float, val baseline: Float, val width: Float,
    val text: String, val start: Int, val length: Int,
) {
    internal val place = Place(scroller, style, x.toRawBits(), baseline.toRawBits())

    /** Whether this run's text begins with all of `other`'s. */
    fun extends(other: FadeRun) =
        other.length <= length && text.regionMatches(start, other.text, other.start, other.length)

    internal data class Place(val scroller: Int, val style: Int, val x: Int, val baseline: Int)
}

/**
 * A filled or stroked box as the fade compares it: its bottom is left out,
 * so a box that only grows taller, such as a streaming code block's, keeps
 * its place.
 */
data class FadeRect(val scroller: Int, val left: Float, val top: Float, val right: Float, val fill: Int?, val stroke: Int?)

/**
 * A run's fade: from `from` points past the run's start up to the next
 * segment (or the run's end), its text appeared at `start` milliseconds.
 * `SHOWN` marks text that was already shown in full.
 */
data class FadeSegment(val from: Float, val start: Long)

/**
 * A row's fade: segments for each run (null for a run shown in full) and a
 * start time for each box (`SHOWN` for a box shown in full), over `duration`
 * milliseconds.
 */
class RowFade(val runs: Array<List<FadeSegment>?>, val rects: LongArray, val duration: Long) {
    /** How opaque text or a box that appeared at `start` is at `now`, from 0 to 1. */
    fun alpha(start: Long, now: Long): Float =
        if (start == StreamFade.SHOWN || duration <= 0L) 1f else ((now - start).toFloat() / duration).coerceIn(0f, 1f)

    /** Whether anything is still fading at `now`. */
    fun active(now: Long): Boolean =
        runs.any { segments -> segments != null && segments.any { alpha(it.start, now) < 1f } } ||
            rects.any { alpha(it, now) < 1f }
}

object StreamFade {
    /** The iOS painter's fade, in milliseconds (`NativeTranscriptView.fade`, 0.18 s). */
    const val DURATION_MS = 180L
    const val SHOWN = Long.MIN_VALUE

    /**
     * The fade for a row's new display list, `runs` and `rects`, that
     * replaces `oldRuns` and `oldRects`, whose fade was `old` (null when all
     * of it was shown). A run at the same place, in the same style, that
     * begins with an old run's text keeps that text's state and fades in only
     * what follows it; any other run fades in whole. A box whose fill,
     * stroke, left, top, and width match an old one keeps its state; others
     * fade in. Text or boxes whose fade has finished become shown, so shown
     * text never fades again. Returns null when nothing fades, or when
     * `duration` is not positive (animations are off).
     */
    fun carry(oldRuns: List<FadeRun>, oldRects: List<FadeRect>, old: RowFade?, runs: List<FadeRun>, rects: List<FadeRect>,
              now: Long, duration: Long): RowFade? {
        if (duration <= 0L) return null
        val byPlace = HashMap<FadeRun.Place, Int>(oldRuns.size * 2)
        oldRuns.forEachIndexed { index, run -> byPlace.putIfAbsent(run.place, index) }
        var fading = false
        val runFades = arrayOfNulls<List<FadeSegment>>(runs.size)
        runs.forEachIndexed { index, run ->
            val previous = byPlace[run.place]?.let { oldRuns[it] }
            val segments: List<FadeSegment>? = when {
                previous == null -> listOf(FadeSegment(0f, now))
                run.extends(previous) -> {
                    val kept = settle(old?.runs?.getOrNull(byPlace[run.place]!!), old, now)
                    if (run.length > previous.length) (kept ?: listOf(FadeSegment(0f, SHOWN))) + FadeSegment(previous.width, now)
                    else kept
                }
                // Text that shrank to a prefix of what was shown is still shown.
                previous.extends(run) -> settle(old?.runs?.getOrNull(byPlace[run.place]!!), old, now)
                else -> listOf(FadeSegment(0f, now))
            }
            val merged = segments?.let { merge(it) }
            runFades[index] = merged
            if (merged != null) fading = true
        }
        val rectPlaces = HashMap<FadeRect, Int>(oldRects.size * 2)
        oldRects.forEachIndexed { index, rect -> rectPlaces.putIfAbsent(rect, index) }
        val rectStarts = LongArray(rects.size) { index ->
            val match = rectPlaces[rects[index]]
            val start = when (match) {
                null -> now
                else -> old?.rects?.getOrNull(match)?.takeIf { old.alpha(it, now) < 1f } ?: SHOWN
            }
            if (start != SHOWN) fading = true
            start
        }
        return if (fading) RowFade(runFades, rectStarts, duration) else null
    }

    /** An old run's segments with finished fades marked shown, or null when all of it is shown. */
    private fun settle(segments: List<FadeSegment>?, old: RowFade?, now: Long): List<FadeSegment>? {
        if (segments == null || old == null) return null
        val settled = segments.map { if (old.alpha(it.start, now) >= 1f) FadeSegment(it.from, SHOWN) else it }
        return merge(settled)
    }

    /** Joins neighbours with the same start; null when everything is shown. */
    private fun merge(segments: List<FadeSegment>): List<FadeSegment>? {
        val out = ArrayList<FadeSegment>(segments.size)
        for (segment in segments) {
            val last = out.lastOrNull()
            if (last != null && last.start == segment.start) continue
            if (last != null && segment.from <= last.from) { out[out.size - 1] = FadeSegment(last.from, segment.start); continue }
            out.add(segment)
        }
        return if (out.all { it.start == SHOWN }) null else out
    }
}
