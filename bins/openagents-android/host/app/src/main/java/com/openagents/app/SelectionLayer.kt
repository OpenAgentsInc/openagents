// The transcript's in-place selection chrome: two drag handles over the list
// and the system's floating text toolbar with Copy and Select All. Each row
// keeps its own selection (RowView); this layer follows the one selected row,
// as bins/coder-ios/host/App/NativeTranscriptPainter.swift does with its
// handles and edit menu.
package com.openagents.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Rect
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.widget.FrameLayout
import androidx.recyclerview.widget.RecyclerView

internal class SelectionLayer(private val root: FrameLayout, private val list: RecyclerView) {
    private val density = root.resources.displayMetrics.density
    private val startHandle = Handle(root.context, true)
    private val endHandle = Handle(root.context, false)
    /** The row whose text is selected. */
    var row: RowView? = null; private set
    private var mode: ActionMode? = null
    private var dragging = false
    /** Where the finger holds the handle, relative to the caret's middle, in pixels. */
    private var grabX = 0f
    private var grabY = 0f

    init {
        for (handle in listOf(startHandle, endHandle)) {
            handle.visibility = View.GONE
            root.addView(handle, FrameLayout.LayoutParams(handle.size, handle.size))
        }
    }

    /** A row's selection started, changed, or ended. */
    fun changed(row: RowView) {
        if (row.selection != null) {
            val previous = this.row
            if (previous !== row) previous?.clearSelection()
            this.row = row
            place()
            if (!dragging) showMenu() else mode?.invalidateContentRect()
        } else if (this.row === row) {
            this.row = null
            place()
            mode?.finish()
        }
    }

    /** Ends the selection, as a tap elsewhere does. */
    fun clear() { row?.clearSelection() }

    /** The list started or stopped scrolling. */
    fun scrolling(moving: Boolean) {
        if (row == null) return
        if (moving) mode?.finish() else showMenu()
    }

    /** The row's top-left corner in the layer, in pixels, while the list shows it. */
    private fun origin(row: RowView): Pair<Float, Float>? {
        if (row.parent !== list || !row.isAttachedToWindow) return null
        return (list.left + row.left).toFloat() to (list.top + row.top).toFloat()
    }

    /** Places the handles at the selection's ends, or hides them. */
    fun place() {
        val row = row
        val selection = row?.selection
        val origin = row?.let { origin(it) }
        for ((handle, position) in listOf(startHandle to selection?.first, endHandle to selection?.second)) {
            val caret = if (origin != null && position != null) row.caret(position) else null
            if (caret == null || origin == null) { handle.visibility = View.GONE; continue }
            val x = origin.first + caret.x * density
            val bottom = origin.second + caret.bottom * density
            if (bottom < list.top || bottom > list.bottom) { handle.visibility = View.GONE; continue }
            handle.translationX = x - handle.hotspot
            handle.translationY = bottom
            handle.visibility = View.VISIBLE
        }
        mode?.invalidateContentRect()
    }

    private fun showMenu() {
        val row = row ?: return
        if (row.selection == null || origin(row) == null) return
        mode?.let { it.invalidateContentRect(); return }
        mode = row.startActionMode(object : ActionMode.Callback2() {
            override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
                menu.add(Menu.NONE, android.R.id.copy, 0, android.R.string.copy)
                menu.add(Menu.NONE, android.R.id.selectAll, 1, android.R.string.selectAll)
                return true
            }
            override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false
            override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
                val selected = this@SelectionLayer.row ?: return false
                when (item.itemId) {
                    android.R.id.copy -> {
                        selected.clipboard(selected.selectedText())
                        selected.clearSelection()
                    }
                    android.R.id.selectAll -> selected.selectAll()
                    else -> return false
                }
                return true
            }
            override fun onDestroyActionMode(ended: ActionMode) { if (mode === ended) mode = null }
            override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) {
                val box = (view as? RowView)?.selectionBounds()
                if (box == null) { outRect.set(0, 0, view.width, view.height); return }
                outRect.set((box.left * density).toInt(), (box.top * density).toInt(),
                    Math.ceil((box.right * density).toDouble()).toInt(), Math.ceil((box.bottom * density).toDouble()).toInt())
            }
        }, ActionMode.TYPE_FLOATING)
    }

    private fun drag(handle: Handle, event: MotionEvent): Boolean {
        val row = row ?: return false
        val selection = row.selection ?: return false
        val origin = origin(row) ?: return false
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                val caret = row.caret(if (handle.start) selection.first else selection.second) ?: return false
                // Aim at the middle of the line, not at the finger on the handle.
                grabX = origin.first + caret.x * density - event.rawX
                grabY = origin.second + (caret.top + caret.bottom) / 2 * density - event.rawY
                dragging = true
                mode?.finish()
                handle.parent?.requestDisallowInterceptTouchEvent(true)
            }
            MotionEvent.ACTION_MOVE -> {
                val x = (event.rawX + grabX - origin.first) / density
                val y = (event.rawY + grabY - origin.second) / density
                val position = row.position(x, y) ?: return true
                // A handle never passes the other one, so the selection keeps a character.
                if (handle.start && position < selection.second) row.select(position, selection.second)
                if (!handle.start && position > selection.first) row.select(selection.first, position)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                dragging = false
                if (event.actionMasked == MotionEvent.ACTION_UP) handle.performClick()
                showMenu()
            }
        }
        return true
    }

    /**
     * A selection handle: a drop whose point is at the caret, below the
     * line, as Android draws text handles, in the selection's color.
     */
    private inner class Handle(context: Context, val start: Boolean) : View(context) {
        val size = context.dp(36)
        private val radius = context.dpf(11f)
        /** The point's x in the view: the drop hangs left of the start and right of the end. */
        val hotspot = if (start) size * 3f / 4 else size / 4f
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.SELECTION_HANDLE }

        init {
            contentDescription = if (start) "Selection start" else "Selection end"
            setOnTouchListener { _, event -> drag(this, event) }
        }

        override fun onDraw(canvas: Canvas) {
            val cx = if (start) hotspot - radius else hotspot + radius
            canvas.drawCircle(cx, radius, radius, paint)
            if (start) canvas.drawRect(hotspot - radius, 0f, hotspot, radius, paint)
            else canvas.drawRect(hotspot, 0f, hotspot + radius, radius, paint)
        }
    }
}
