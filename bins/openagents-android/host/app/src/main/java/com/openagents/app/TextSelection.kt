// In-place text selection over a painted transcript row. Rust places every
// text run (`rust_native::layout::display::Run`); this file finds carets,
// hit positions, highlight boxes, and the selected text from those runs. It
// has no Android dependencies, so its unit tests run on the JVM. It mirrors
// NativeRowModel's text positions in
// bins/coder-ios/host/App/NativeTranscriptPainter.swift.
package com.openagents.app

/** A place in a row's text: a run and a UTF-16 offset within it. */
data class TextPosition(val run: Int, val offset: Int) : Comparable<TextPosition> {
    override fun compareTo(other: TextPosition) =
        if (run != other.run) run.compareTo(other.run) else offset.compareTo(other.offset)
}

/** A rectangle in unscrolled row coordinates, in points. */
data class TextBox(val left: Float, val top: Float, val right: Float, val bottom: Float)

/** A caret in unscrolled row coordinates: x, and the line's top and bottom. */
data class Caret(val x: Float, val top: Float, val bottom: Float)

/**
 * One text run as Rust placed it, with the font's ascent and descent and the
 * run's caret stops. `carets` returns `length + 1` advances from the run's
 * start, one for each UTF-16 offset, or null when the run can't be measured.
 */
class SelectableRun(
    val text: Int, val start: Int, val length: Int,
    val x: Float, val baseline: Float, val width: Float,
    val ascent: Float, val descent: Float,
    val scroller: Int,
    /** A truncated run's width limit: carets past it stay at it. */
    private val limit: Float? = null,
    private val measure: () -> FloatArray?,
) {
    /** Caret stops scaled so the run's end is Rust's measured width. */
    val carets: FloatArray? by lazy {
        val raw = measure()?.takeIf { it.size == length + 1 } ?: return@lazy null
        val total = raw[length]
        FloatArray(length + 1) { i ->
            val x = if (total > 0f && width > 0f) raw[i] * (width / total) else raw[i]
            if (limit != null) minOf(x, maxOf(0f, limit)) else x
        }
    }
    /**
     * Offsets a caret may sit at. A cluster's advance falls on its first
     * UTF-16 unit and the rest advance nothing, so a caret never lands inside
     * a surrogate pair, a ligature, or before a combining mark.
     */
    val stops: BooleanArray by lazy {
        val c = carets
        BooleanArray(length + 1) { i -> i == 0 || i == length || c == null || c[i + 1] > c[i] }
    }
}

/** The selectable text of one row. */
class SelectableText(private val texts: List<String>, private val runs: List<SelectableRun?>) {

    private fun usable(index: Int): SelectableRun? =
        runs.getOrNull(index)?.takeIf { it.carets != null && it.text < texts.size && it.length > 0 &&
            it.start >= 0 && it.start + it.length <= texts[it.text].length }

    val hasText: Boolean get() = runs.indices.any { usable(it) != null }

    /** The scroller a run belongs to, or -1. */
    fun scroller(ofRun: Int): Int = runs.getOrNull(ofRun)?.scroller ?: -1

    /** The first and last positions of the row's text. */
    fun textBounds(): Pair<TextPosition, TextPosition>? {
        val first = runs.indices.firstOrNull { usable(it) != null } ?: return null
        val last = runs.indices.last { usable(it) != null }
        return TextPosition(first, 0) to TextPosition(last, runs[last]!!.length)
    }

    fun caret(position: TextPosition): Caret? {
        val run = usable(position.run) ?: return null
        val carets = run.carets!!
        val x = run.x + carets[position.offset.coerceIn(0, run.length)]
        return Caret(x, run.baseline - run.ascent, run.baseline + run.descent)
    }

    /**
     * The text position nearest `(x, y)` among the runs of `scroller`: the
     * nearest line first, then the nearest run on it, then the nearest caret
     * stop.
     */
    fun position(x: Float, y: Float, scroller: Int): TextPosition? {
        var best = -1
        var bestScore = Float.POSITIVE_INFINITY
        for (index in runs.indices) {
            val run = usable(index) ?: continue
            if (run.scroller != scroller) continue
            val top = run.baseline - run.ascent
            val bottom = run.baseline + run.descent
            val dy = if (y < top) top - y else if (y > bottom) y - bottom else 0f
            val dx = if (x < run.x) run.x - x else if (x > run.x + run.width) x - run.x - run.width else 0f
            val score = dy * 1_000f + dx
            if (score < bestScore) { bestScore = score; best = index }
        }
        if (best < 0) return null
        val run = runs[best]!!
        val carets = run.carets!!
        val local = x - run.x
        var offset = 0
        var distance = Float.POSITIVE_INFINITY
        for (i in 0..run.length) {
            if (!run.stops[i]) continue
            val d = kotlin.math.abs(carets[i] - local)
            if (d < distance) { distance = d; offset = i }
        }
        return TextPosition(best, offset)
    }

    /** The selection's highlight boxes by scroller (-1 for the row), unscrolled. */
    fun highlights(start: TextPosition, end: TextPosition): Map<Int, List<TextBox>> {
        val out = HashMap<Int, MutableList<TextBox>>()
        if (start >= end) return out
        for (index in start.run..minOf(end.run, runs.size - 1)) {
            val run = usable(index) ?: continue
            val carets = run.carets!!
            val a = if (index == start.run) start.offset.coerceIn(0, run.length) else 0
            val b = if (index == end.run) end.offset.coerceIn(0, run.length) else run.length
            if (b <= a) continue
            val x0 = run.x + carets[a]
            val x1 = maxOf(run.x + carets[b], x0 + 1f)
            out.getOrPut(run.scroller) { ArrayList() }
                .add(TextBox(x0, run.baseline - run.ascent, x1, run.baseline + run.descent))
        }
        return out
    }

    /**
     * The selected text. Pieces of one paragraph join with the text between
     * them; separate paragraphs join with a tab on one baseline, else a line
     * break.
     */
    fun text(start: TextPosition, end: TextPosition): String {
        if (start >= end) return ""
        val out = StringBuilder()
        var previous: Triple<Int, Int, Float>? = null
        for (index in start.run..minOf(end.run, runs.size - 1)) {
            val run = usable(index) ?: continue
            val a = if (index == start.run) start.offset.coerceIn(0, run.length) else 0
            val b = if (index == end.run) end.offset.coerceIn(0, run.length) else run.length
            if (b <= a) continue
            val text = texts[run.text]
            previous?.let { (textIndex, endOffset, baseline) ->
                if (textIndex == run.text && endOffset <= run.start + a) {
                    out.append(text, endOffset, run.start + a)
                } else {
                    out.append(if (kotlin.math.abs(baseline - run.baseline) < 0.5f) '\t' else '\n')
                }
            }
            out.append(text, run.start + a, run.start + b)
            previous = Triple(run.text, run.start + b, run.baseline)
        }
        return out.toString()
    }
}
