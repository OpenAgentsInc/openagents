// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.Shader
import uniffi.openagents_core.BoxStyle
import uniffi.openagents_core.ColorRole
import uniffi.openagents_core.Decoration
import uniffi.openagents_core.Fade
import uniffi.openagents_core.FadeEdge
import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.StyleDesc
import uniffi.openagents_core.TextRun

/** B1 supplies oa-tokens colors; changing appearance must not trigger Rust layout. */
fun interface TranscriptPalette { fun color(role: ColorRole): Int }

/** C2/C3 supply native widgets inside Rust's rectangles, without changing geometry. */
fun interface TranscriptWidgets {
    fun draw(canvas: Canvas, row: RowDisplay, scroller: UInt?)
}

class TranscriptPainter(
    private val fonts: TranscriptFonts,
    private val palette: TranscriptPalette,
    private val widgets: TranscriptWidgets? = null,
) {
    private data class Key(val key: ULong, val version: ULong, val width: Float)
    private data class Line(val run: TextRun, val shaped: AndroidShapedText)
    private var styles = emptyList<StyleDesc>()
    private var styleTable = emptyMap<UShort, StyleDesc>()
    private val prepared = object : LinkedHashMap<Key, List<Line>>(16, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, List<Line>>?) = size > 96
    }
    private val brush = Paint(Paint.ANTI_ALIAS_FLAG)

    fun setStyles(next: List<StyleDesc>) {
        if (styles != next) {
            styles = next
            styleTable = next.associateBy { it.id }
            prepared.clear()
        }
    }

    fun clear() { prepared.clear(); styles = emptyList(); styleTable = emptyMap() }

    fun draw(canvas: Canvas, row: RowDisplay, density: Float, horizontalOffset: (UInt) -> Float) {
        val key = Key(row.key, row.version, row.width)
        val lines = prepared[key] ?: row.runs.map { run ->
            val start = run.start.toInt()
            val end = start + run.len.toInt()
            require(start >= 0 && end >= start && end <= row.text.length)
            require(start == 0 || start == row.text.length || !row.text[start].isLowSurrogate())
            require(end == row.text.length || !row.text[end].isLowSurrogate())
            val style = styleTable.getValue(run.style)
            Line(run, AndroidShapedText(row.text.substring(start, end), fonts.paint(style.face, style.size, style.ligatures)))
        }.also { prepared[key] = it }
        drawLayer(canvas, row, lines, null, density)
        row.scrollers.forEachIndexed { index, scroller ->
            val save = canvas.save()
            canvas.clipRect(scroller.x, scroller.y, scroller.x + scroller.w, scroller.y + scroller.h)
            val offset = horizontalOffset(index.toUInt()).coerceIn(0f, (scroller.contentWidth - scroller.w).coerceAtLeast(0f))
            canvas.translate(scroller.x - offset, scroller.y)
            drawLayer(canvas, row, lines, index.toUInt(), density)
            canvas.restoreToCount(save)
        }
    }

    private fun drawLayer(canvas: Canvas, row: RowDisplay, lines: List<Line>, layer: UInt?, density: Float) {
        val hairline = 1f / density
        for (box in row.boxes) {
            if (box.scroller != layer || box.w <= 0 || box.h <= 0) continue
            brush.color = palette.color(box.color)
            val inset = if (box.style == BoxStyle.HAIRLINE) hairline / 2f else 0f
            brush.style = if (box.style == BoxStyle.HAIRLINE) Paint.Style.STROKE else Paint.Style.FILL
            brush.strokeWidth = hairline
            val radius = (box.radius - inset).coerceAtLeast(0f)
            if (box.w >= inset * 2 && box.h >= inset * 2) {
                canvas.drawRoundRect(box.x + inset, box.y + inset, box.x + box.w - inset, box.y + box.h - inset, radius, radius, brush)
            }
        }
        val fades = row.fades.filter { it.scroller == layer }
        val assigned = lines.filter { it.run.scroller == layer }.groupBy { line ->
            fades.indexOfFirst { fade -> line.run.baseline > fade.y && line.run.baseline <= fade.y + fade.h + .5f }
        }
        assigned[-1].orEmpty().forEach { drawLine(canvas, it, hairline) }
        fades.forEachIndexed { index, fade ->
            val group = assigned[index].orEmpty()
            if (group.isEmpty() || fade.w <= 0 || fade.h <= 0) return@forEachIndexed
            drawFaded(canvas, fade) { group.forEach { drawLine(canvas, it, hairline) } }
        }
        widgets?.draw(canvas, row, layer)
    }

    private fun drawLine(canvas: Canvas, line: Line, hairline: Float) {
        val run = line.run
        line.shaped.paint.color = palette.color(run.color)
        line.shaped.draw(canvas, run.x, run.baseline)
        if (run.decoration != Decoration.NONE) {
            brush.style = Paint.Style.FILL
            brush.color = line.shaped.paint.color
            val y = run.baseline + if (run.decoration == Decoration.UNDERLINE) 2f else -5f
            canvas.drawRect(run.x, y, run.x + run.width, y + maxOf(1f, hairline), brush)
        }
    }

    private fun drawFaded(canvas: Canvas, fade: Fade, draw: () -> Unit) {
        val save = canvas.save()
        val clip = canvas.clipBounds
        if (fade.edge == FadeEdge.TRAILING) canvas.clipRect(clip.left.toFloat(), clip.top.toFloat(), fade.x + fade.w, clip.bottom.toFloat())
        else canvas.clipRect(clip.left.toFloat(), clip.top.toFloat(), clip.right.toFloat(), fade.y + fade.h)
        val layer = canvas.saveLayer(null, null)
        draw()
        brush.style = Paint.Style.FILL
        brush.color = Color.BLACK
        brush.shader = if (fade.edge == FadeEdge.TRAILING) {
            LinearGradient(fade.x, 0f, fade.x + fade.w, 0f, Color.BLACK, Color.TRANSPARENT, Shader.TileMode.CLAMP)
        } else LinearGradient(0f, fade.y, 0f, fade.y + fade.h, Color.BLACK, Color.TRANSPARENT, Shader.TileMode.CLAMP)
        brush.xfermode = PorterDuffXfermode(PorterDuff.Mode.DST_IN)
        canvas.drawRect(clip, brush)
        brush.shader = null
        brush.xfermode = null
        canvas.restoreToCount(layer)
        canvas.restoreToCount(save)
    }
}
