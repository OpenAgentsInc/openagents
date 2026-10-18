// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.Widget
import uniffi.openagents_core.WidgetKind

/** A stable selection point in a Rust row, with an Android UTF-16 text offset. */
data class TranscriptTextPoint(val rowKey: ULong, val utf16Offset: Int)

data class TranscriptTextSelection(val anchor: TranscriptTextPoint, val focus: TranscriptTextPoint)

/** Keeps a user's selection attached to row identity as Rust publishes new frames. */
class TranscriptSelectionState {
    var selection: TranscriptTextSelection? = null
        private set

    fun select(anchor: TranscriptTextPoint, focus: TranscriptTextPoint) {
        require(anchor.utf16Offset >= 0 && focus.utf16Offset >= 0)
        selection = TranscriptTextSelection(anchor, focus)
    }

    fun clear() { selection = null }

    /** UTF-16 range for one visible row, suitable for drawing a native selection overlay. */
    fun rangeForRow(frame: TranscriptFrame, rowKey: ULong): Pair<Int, Int>? {
        val current = selection ?: return null
        val anchorIndex = frame.indexOf(current.anchor.rowKey)?.toInt() ?: return null
        val focusIndex = frame.indexOf(current.focus.rowKey)?.toInt() ?: return null
        val rowIndex = frame.indexOf(rowKey)?.toInt() ?: return null
        val first = if (anchorIndex < focusIndex || anchorIndex == focusIndex && current.anchor.utf16Offset <= current.focus.utf16Offset) {
            current.anchor
        } else current.focus
        val last = if (first == current.anchor) current.focus else current.anchor
        val firstIndex = minOf(anchorIndex, focusIndex)
        val lastIndex = maxOf(anchorIndex, focusIndex)
        if (rowIndex !in firstIndex..lastIndex) return null
        val display = frame.display(rowIndex.toUInt()) ?: return null
        val start = if (rowIndex == firstIndex) boundary(display.text, first.utf16Offset, roundUp = false) else 0
        val end = if (rowIndex == lastIndex) boundary(display.text, last.utf16Offset, roundUp = true) else display.text.length
        return start to end
    }

    /** Preserve both row anchors across streaming updates; clear if either row disappeared. */
    fun reconcile(frame: TranscriptFrame) {
        val current = selection ?: return
        val anchorIndex = frame.indexOf(current.anchor.rowKey)?.toInt() ?: return clear()
        val focusIndex = frame.indexOf(current.focus.rowKey)?.toInt() ?: return clear()
        val anchorIsStart = anchorIndex < focusIndex ||
            anchorIndex == focusIndex && current.anchor.utf16Offset <= current.focus.utf16Offset
        val anchor = normalize(frame, current.anchor, roundUp = !anchorIsStart) ?: return clear()
        val focus = normalize(frame, current.focus, roundUp = anchorIsStart) ?: return clear()
        selection = TranscriptTextSelection(anchor, focus)
    }

    /** Return the selected visible text, joining selected rows with newlines. */
    fun selectedText(frame: TranscriptFrame): String? {
        val current = selection ?: return null
        val anchorIndex = frame.indexOf(current.anchor.rowKey)?.toInt() ?: return null
        val focusIndex = frame.indexOf(current.focus.rowKey)?.toInt() ?: return null
        val first = if (anchorIndex < focusIndex || anchorIndex == focusIndex && current.anchor.utf16Offset <= current.focus.utf16Offset) {
            current.anchor
        } else current.focus
        val last = if (first == current.anchor) current.focus else current.anchor
        val firstIndex = minOf(anchorIndex, focusIndex)
        val lastIndex = maxOf(anchorIndex, focusIndex)
        val pieces = ArrayList<String>(lastIndex - firstIndex + 1)
        for (index in firstIndex..lastIndex) {
            val row = frame.placement(index.toUInt()) ?: return null
            val display = frame.display(index.toUInt()) ?: return null
            val start = if (row.key == first.rowKey) boundary(display.text, first.utf16Offset, roundUp = false) else 0
            val end = if (row.key == last.rowKey) boundary(display.text, last.utf16Offset, roundUp = true) else display.text.length
            if (start > end) return null
            pieces += display.text.substring(start, end)
        }
        return pieces.joinToString("\n")
    }

    private fun normalize(frame: TranscriptFrame, point: TranscriptTextPoint, roundUp: Boolean): TranscriptTextPoint? {
        val index = frame.indexOf(point.rowKey) ?: return null
        val display = frame.display(index) ?: return null
        return point.copy(utf16Offset = boundary(display.text, point.utf16Offset, roundUp))
    }

    private fun boundary(text: String, offset: Int, roundUp: Boolean): Int {
        var value = offset.coerceIn(0, text.length)
        if (value in 1 until text.length && text[value].isLowSurrogate()) {
            value += if (roundUp) 1 else -1
        }
        return value
    }
}

sealed interface TranscriptHitTarget {
    data class Link(val url: String) : TranscriptHitTarget
    data class Widget(val rowKey: ULong, val widgetId: UInt, val kind: WidgetKind) : TranscriptHitTarget
}

/** Resolves taps against Rust's hit rectangles without deriving transcript geometry in Kotlin. */
object TranscriptHitTester {
    fun hit(
        frame: TranscriptFrame,
        x: Float,
        y: Float,
        verticalOffset: Float,
        horizontalOffsets: Map<UInt, Float> = emptyMap(),
    ): TranscriptHitTarget? {
        if (!x.isFinite() || !y.isFinite() || !verticalOffset.isFinite()) return null
        val absoluteY = y + verticalOffset
        val index = frame.indexAt(absoluteY) ?: return null
        val placement = frame.placement(index) ?: return null
        val display = frame.display(index) ?: return null
        val rowY = absoluteY - placement.y

        display.widgets.asReversed().firstNotNullOfOrNull { widget ->
            if (!widget.isInteractive()) return@firstNotNullOfOrNull null
            if (widget.contains(x, rowY, display, horizontalOffsets)) {
                TranscriptHitTarget.Widget(placement.key, widget.id, widget.kind)
            } else null
        }?.let { return it }

        display.links.asReversed().firstNotNullOfOrNull { link ->
            val point = contentPoint(x, rowY, link.scroller, display, horizontalOffsets) ?: return@firstNotNullOfOrNull null
            if (point.first >= link.x && point.first < link.x + link.w && point.second >= link.y && point.second < link.y + link.h) {
                TranscriptHitTarget.Link(link.url)
            } else null
        }?.let { return it }
        return null
    }

    private fun Widget.isInteractive() = when (kind) {
        WidgetKind.CopyCode -> true
        is WidgetKind.Disclosure, is WidgetKind.Image, is WidgetKind.Detail,
        is WidgetKind.ToolToggle, is WidgetKind.Visual, is WidgetKind.LabMedia,
        is WidgetKind.LabOpen, is WidgetKind.LabNote, is WidgetKind.LabVersions -> true
        else -> false
    }

    private fun Widget.contains(
        x: Float,
        y: Float,
        display: RowDisplay,
        horizontalOffsets: Map<UInt, Float>,
    ): Boolean {
        val point = contentPoint(x, y, scroller, display, horizontalOffsets) ?: return false
        return point.first >= this.x && point.first < this.x + w &&
            point.second >= this.y && point.second < this.y + h
    }

    private fun contentPoint(
        x: Float,
        y: Float,
        scrollerIndex: UInt?,
        display: RowDisplay,
        horizontalOffsets: Map<UInt, Float>,
    ): Pair<Float, Float>? {
        if (scrollerIndex == null) return x to y
        val scroller = display.scrollers.getOrNull(scrollerIndex.toInt()) ?: return null
        if (x < scroller.x || x >= scroller.x + scroller.w || y < scroller.y || y >= scroller.y + scroller.h) return null
        val offset = (horizontalOffsets[scrollerIndex] ?: 0f).coerceIn(0f, (scroller.contentWidth - scroller.w).coerceAtLeast(0f))
        return (x - scroller.x + offset) to (y - scroller.y)
    }
}
