// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.icu.text.BreakIterator
import java.util.Locale
import kotlin.math.abs
import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.TextRun

/** Borrowed geometry: no frame handle, layout, or viewport is duplicated here. */
data class TranscriptInteractionSnapshot(
    val frame: TranscriptFrame,
    val top: Float,
    val height: Float,
    val horizontal: (ULong, UInt) -> Float,
)
data class TranscriptSelectionRect(val left: Float, val top: Float, val right: Float, val bottom: Float)
data class TranscriptSelectionCaret(val x: Float, val top: Float, val bottom: Float)

object AndroidTranscriptBoundaries : TranscriptTextBoundaries {
    override fun boundary(text: String, offset: Int, roundUp: Boolean): Int {
        val at = offset.coerceIn(0, text.length)
        val iterator = BreakIterator.getCharacterInstance(Locale.ROOT)
        iterator.setText(text)
        if (iterator.isBoundary(at)) return at
        return if (roundUp) iterator.following(at) else iterator.preceding(at)
    }
    fun word(text: String, offset: Int): IntRange? {
        if (text.isEmpty()) return null
        val at = offset.coerceIn(0, text.lastIndex)
        val words = BreakIterator.getWordInstance(Locale.ROOT)
        words.setText(text)
        val end = words.following(at)
        val start = words.preceding(end)
        if (start < 0 || end <= start) return null
        return boundary(text, start, false) until boundary(text, end, true)
    }
}

/** Uses C1's exact font/shaping context inside Rust-selected runs; never wraps text. */
class AndroidTranscriptSelectionGeometry(private val fonts: TranscriptFonts) {
    private data class Line(val run: TextRun, val shaped: AndroidShapedText, val left: Float, val top: Float, val bottom: Float)
    private fun lines(snapshot: TranscriptInteractionSnapshot, row: RowDisplay): List<Line> {
        val styles = snapshot.frame.styles().associateBy { it.id }
        return row.runs.take(8192).mapNotNull { run ->
            val start = run.start.toLong(); val end = start + run.len.toLong()
            val style = styles[run.style] ?: return@mapNotNull null
            if (start < 0 || end > row.text.length || end < start || !run.x.isFinite() || !run.baseline.isFinite()) return@mapNotNull null
            val shaped = AndroidShapedText(row.text.substring(start.toInt(), end.toInt()), fonts.paint(style.face, style.size, style.ligatures))
            val scroller = run.scroller?.let { row.scrollers.getOrNull(it.toInt()) } ?: if (run.scroller == null) null else return@mapNotNull null
            val offset = run.scroller?.let { snapshot.horizontal(row.key, it) } ?: 0f
            if (!offset.isFinite()) return@mapNotNull null
            val x = run.x + (scroller?.x ?: 0f) - offset
            val y = scroller?.y ?: 0f
            val metrics = shaped.paint.fontMetrics
            Line(run, shaped, x, run.baseline + y + metrics.ascent, run.baseline + y + metrics.descent)
        }
    }
    private fun caretX(line: Line, offset: Int): Float {
        val local = (offset - line.run.start.toInt()).coerceIn(0, line.shaped.text.length)
        var x = line.left
        for (direction in line.shaped.runs) {
            if (local in direction.start..direction.end) {
                val advance = line.shaped.paint.getRunAdvance(line.shaped.chars, direction.start, direction.end,
                    direction.start, direction.end, direction.rtl, local)
                return x + if (direction.rtl) direction.width - advance else advance
            }
            x += direction.width
        }
        return x
    }
    fun hit(snapshot: TranscriptInteractionSnapshot, x: Float, y: Float): TranscriptTextPoint? {
        if (!x.isFinite() || !y.isFinite()) return null
        val index = snapshot.frame.indexAt(y + snapshot.top) ?: return null
        val placement = snapshot.frame.placement(index) ?: return null
        val row = snapshot.frame.display(index) ?: return null
        val localY = y + snapshot.top - placement.y
        val line = lines(snapshot, row).filter { line ->
            val scroller = line.run.scroller?.let { row.scrollers.getOrNull(it.toInt()) }
            scroller == null || x in scroller.x..(scroller.x + scroller.w) && localY in scroller.y..(scroller.y + scroller.h)
        }.minByOrNull { line ->
            val dy = if (localY < line.top) line.top - localY else if (localY > line.bottom) localY - line.bottom else 0f
            dy * 10000f + abs(x - (line.left + line.shaped.width / 2))
        } ?: return null
        var left = line.left
        var closest = line.run.start.toInt()
        var distance = Float.POSITIVE_INFINITY
        for (direction in line.shaped.runs) {
            val advance = (if (direction.rtl) left + direction.width - x else x - left).coerceIn(0f, direction.width)
            val offset = line.shaped.paint.getOffsetForAdvance(line.shaped.chars, direction.start, direction.end,
                direction.start, direction.end, direction.rtl, advance)
            val position = caretX(line, line.run.start.toInt() + offset)
            if (abs(position - x) < distance) { distance = abs(position - x); closest = line.run.start.toInt() + offset }
            left += direction.width
        }
        return TranscriptTextPoint(row.key, AndroidTranscriptBoundaries.boundary(row.text, closest, false))
    }
    fun word(snapshot: TranscriptInteractionSnapshot, point: TranscriptTextPoint): TranscriptTextSelection? {
        val row = snapshot.frame.indexOf(point.rowKey)?.let(snapshot.frame::display) ?: return null
        val range = AndroidTranscriptBoundaries.word(row.text, point.utf16Offset) ?: return null
        return TranscriptTextSelection(TranscriptTextPoint(point.rowKey, range.first), TranscriptTextPoint(point.rowKey, range.last + 1))
    }
    fun caret(snapshot: TranscriptInteractionSnapshot, point: TranscriptTextPoint): TranscriptSelectionCaret? {
        val index = snapshot.frame.indexOf(point.rowKey) ?: return null
        val row = snapshot.frame.display(index) ?: return null
        val placement = snapshot.frame.placement(index) ?: return null
        val line = lines(snapshot, row).firstOrNull { point.utf16Offset in it.run.start.toInt()..(it.run.start.toInt() + it.run.len.toInt()) } ?: return null
        val top = placement.y - snapshot.top
        return TranscriptSelectionCaret(caretX(line, point.utf16Offset), top + line.top, top + line.bottom)
    }
    fun rectangles(snapshot: TranscriptInteractionSnapshot, state: TranscriptSelectionState): List<TranscriptSelectionRect> {
        val rectangles = mutableListOf<TranscriptSelectionRect>()
        for (placement in snapshot.frame.rowsIn(snapshot.top, snapshot.top + snapshot.height).take(128)) {
            val range = state.rangeForRow(snapshot.frame, placement.key) ?: continue
            val row = snapshot.frame.display(placement.index) ?: continue
            for (line in lines(snapshot, row)) {
                for (direction in line.shaped.runs) {
                    val start = maxOf(range.first, line.run.start.toInt() + direction.start)
                    val end = minOf(range.second, line.run.start.toInt() + direction.end)
                    if (start >= end) continue
                    val x1 = caretX(line, start); val x2 = caretX(line, end)
                    val scroller = line.run.scroller?.let { row.scrollers.getOrNull(it.toInt()) }
                    val left = maxOf(minOf(x1, x2), scroller?.x ?: 0f)
                    val right = minOf(maxOf(x1, x2), scroller?.let { it.x + it.w } ?: snapshot.frame.width)
                    val top = maxOf(line.top, scroller?.y ?: 0f) + placement.y - snapshot.top
                    val bottom = minOf(line.bottom, scroller?.let { it.y + it.h } ?: row.height) + placement.y - snapshot.top
                    if (right > left && bottom > top && rectangles.size < 8192) rectangles += TranscriptSelectionRect(left, top, right, bottom)
                }
            }
        }
        return rectangles
    }
}
