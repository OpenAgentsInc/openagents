// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Rect
import android.os.Bundle
import android.view.ActionMode
import android.view.GestureDetector
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.widget.FrameLayout

interface TranscriptSelectionCallbacks {
    fun onSelection(token: TranscriptSelectionToken, range: TranscriptTextSelection?)
    fun onHit(token: TranscriptSelectionToken, target: TranscriptHitTarget)
    fun onSystemTextAction(intent: Intent)
}

/** Wrap C1's view. The registered supplier borrows C1's actual viewport and frame. */
class TranscriptSelectionHost(
    context: Context,
    private val content: View,
    private val snapshot: () -> TranscriptInteractionSnapshot?,
    private val geometry: AndroidTranscriptSelectionGeometry,
    private val callbacks: TranscriptSelectionCallbacks,
    private val selectionColor: Int,
    private val handleColor: Int,
) : FrameLayout(context), AutoCloseable {
    private val density get() = resources.displayMetrics.density
    private var live = true
    private var contentReleased = false
    private var actionMode: ActionMode? = null
    private var menuToken: TranscriptSelectionToken? = null
    private var gestureToken: TranscriptSelectionToken? = null
    private var draggingAnchor: Boolean? = null
    private var longPressed = false
    private val processorTargets = mutableMapOf<Int, ComponentName>()
    private val accessibleRows = mutableMapOf<Int, Pair<TranscriptSelectionToken, ULong>>()
    private var accessibilityFocus: ULong? = null
    private fun accessibilityEvent(id: Int, type: Int) {
        val event = AccessibilityEvent.obtain(type)
        event.packageName = context.packageName
        event.className = android.widget.TextView::class.java.name
        event.setSource(this, id)
        parent?.requestSendAccessibilityEvent(this, event)
    }
    private val model = TranscriptSelectionController(AndroidTranscriptBoundaries) { range ->
        if (range == null) actionMode?.finish() else actionMode?.invalidate()
        overlay.invalidate()
        accessibleRows.clear()
        sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
        callbacks.onSelection(modelToken(), range)
    }
    private fun modelToken(): TranscriptSelectionToken = model.token
    private fun current(): TranscriptInteractionSnapshot? {
        if (!live) return null
        val value = snapshot() ?: return null
        if (value.frame !== model.currentFrame || !value.top.isFinite() || !value.height.isFinite() || value.top < 0 || value.height < 0) return null
        return value
    }
    private val overlay = object : View(context) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        override fun onDraw(canvas: Canvas) {
            val value = current() ?: return
            val selected = model.state.selection ?: return
            canvas.save()
            canvas.translate(content.paddingLeft.toFloat(), content.paddingTop.toFloat())
            canvas.scale(density, density)
            canvas.clipRect(0f, 0f, value.frame.width, value.height)
            paint.color = selectionColor
            geometry.rectangles(value, model.state).forEach { canvas.drawRect(it.left, it.top, it.right, it.bottom, paint) }
            paint.color = handleColor
            for (point in listOf(selected.anchor, selected.focus)) geometry.caret(value, point)?.let {
                canvas.drawRect(it.x - .75f, it.top, it.x + .75f, it.bottom, paint)
                canvas.drawCircle(it.x, it.bottom + 7f, 7f, paint)
            }
            canvas.restore()
        }
    }
    private val detector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(event: MotionEvent) = true
        override fun onLongPress(event: MotionEvent) {
            val token = gestureToken ?: return
            val value = current() ?: return
            if (!model.accepts(token)) return
            val point = geometry.hit(value, x(event.x), y(event.y)) ?: return
            val word = geometry.word(value, point) ?: return
            if (!model.select(token, word.anchor, word.focus)) return
            longPressed = true
            cancelContentTouch(event)
            parent?.requestDisallowInterceptTouchEvent(true)
            showActions()
        }
        override fun onSingleTapUp(event: MotionEvent): Boolean {
            if (model.state.selection != null) { cancelSelection(); return true }
            val token = gestureToken ?: return false
            val value = current() ?: return false
            if (!model.accepts(token)) return false
            val row = value.frame.indexAt(y(event.y) + value.top)?.let(value.frame::placement) ?: return false
            val scrollers = value.frame.display(row.index)?.scrollers.orEmpty()
            val offsets = scrollers.indices.associate { it.toUInt() to value.horizontal(row.key, it.toUInt()) }
            val hit = TranscriptHitTester.hit(value.frame, x(event.x), y(event.y), value.top, offsets) ?: return false
            callbacks.onHit(token, hit)
            return true
        }
    })
    init {
        isSaveEnabled = false
        isFocusable = true
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES
        content.importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
        addView(content, LayoutParams(-1, -1))
        overlay.isSaveEnabled = false
        overlay.importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
        addView(overlay, LayoutParams(-1, -1))
    }
    fun observer(accountConversation: String): TranscriptInteractionObserver = object : TranscriptInteractionObserver {
        override fun beforeFrameReplacement(next: TranscriptFrame) { this@TranscriptSelectionHost.beforeFrameReplacement(next, accountConversation) }
        override fun viewportChanged() = this@TranscriptSelectionHost.viewportChanged()
        override fun beforeFrameClose() { contentReleased = true; retireBorrow() }
    }
    /** Call BEFORE replacing/closing the old C1 frame; never close the borrowed frame here. */
    fun beforeFrameReplacement(next: TranscriptFrame, accountConversation: String): Boolean {
        check(android.os.Looper.myLooper() == android.os.Looper.getMainLooper())
        val accepted = model.submit(next, accountConversation)
        if (accepted) { gestureToken = null; draggingAnchor = null; longPressed = false }
        return accepted
    }
    /** Call on C1 scroll, resize, or horizontal movement, after its viewport changes. */
    fun viewportChanged() { overlay.invalidate(); actionMode?.invalidate(); accessibleRows.clear() }
    fun bookmark() = model.bookmark()
    fun restore(bookmark: TranscriptSelectionBookmark): Boolean = model.restore(model.token, bookmark)
    private fun x(value: Float) = (value - content.paddingLeft) / density
    private fun y(value: Float) = (value - content.paddingTop) / density
    private fun handleAt(event: MotionEvent): Boolean? {
        val selected = model.state.selection ?: return null
        val value = current() ?: return null
        for ((anchor, point) in listOf(true to selected.anchor, false to selected.focus)) {
            val caret = geometry.caret(value, point) ?: continue
            if (kotlin.math.abs(x(event.x) - caret.x) <= 24f && kotlin.math.abs(y(event.y) - caret.bottom - 7f) <= 24f) return anchor
        }
        return null
    }
    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        if (!live || current() == null) return super.dispatchTouchEvent(event)
        if (event.actionMasked == MotionEvent.ACTION_DOWN) {
            gestureToken = model.token
            longPressed = false
            draggingAnchor = handleAt(event)
        }
        val anchor = draggingAnchor
        if (anchor != null) {
            parent?.requestDisallowInterceptTouchEvent(true)
            if (event.actionMasked == MotionEvent.ACTION_MOVE) {
                val token = gestureToken
                val value = current()
                val point = value?.let { geometry.hit(it, x(event.x), y(event.y)) }
                if (token != null && point != null) model.moveHandle(token, anchor, point)
            }
            if (event.actionMasked in listOf(MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL)) {
                draggingAnchor = null; gestureToken = null
                parent?.requestDisallowInterceptTouchEvent(false)
            }
            return true
        }
        val handled = detector.onTouchEvent(event)
        if (longPressed) {
            if (event.actionMasked in listOf(MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL)) {
                longPressed = false; gestureToken = null
                parent?.requestDisallowInterceptTouchEvent(false)
            }
            return true
        }
        if (event.actionMasked == MotionEvent.ACTION_CANCEL) { gestureToken = null; return super.dispatchTouchEvent(event) }
        if (handled && event.actionMasked == MotionEvent.ACTION_UP) {
            cancelContentTouch(event)
            gestureToken = null
            return true
        }
        return super.dispatchTouchEvent(event)
    }
    private fun cancelContentTouch(event: MotionEvent) {
        val cancelled = MotionEvent.obtain(event)
        try { cancelled.action = MotionEvent.ACTION_CANCEL; content.dispatchTouchEvent(cancelled) } finally { cancelled.recycle() }
    }
    private fun showActions() {
        if (actionMode == null) actionMode = startActionMode(actions, ActionMode.TYPE_FLOATING)
        actionMode?.invalidate()
    }
    private fun copy(token: TranscriptSelectionToken): Boolean = model.copy(token) { text ->
        val clipboard = context.getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText(context.getString(android.R.string.copy), text))
    }
    private val actions = object : ActionMode.Callback2() {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu) = live && model.state.selection != null
        override fun onPrepareActionMode(mode: ActionMode, menu: Menu): Boolean {
            menu.clear(); processorTargets.clear(); menuToken = model.token
            val text = model.selectedText(model.token)?.takeIf { it.isNotEmpty() } ?: return false
            menu.add(0, android.R.id.copy, 0, android.R.string.copy).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
            val query = Intent(Intent.ACTION_PROCESS_TEXT).setType("text/plain")
            @Suppress("DEPRECATION")
            val matches = context.packageManager.queryIntentActivities(query, 0).take(8)
            for ((index, resolved) in matches.withIndex()) {
                val info = resolved.activityInfo
                if (!info.exported || info.permission != null) continue
                val id = 1000 + index
                processorTargets[id] = ComponentName(info.packageName, info.name)
                menu.add(0, id, index + 1, resolved.loadLabel(context.packageManager))
            }
            return text.isNotEmpty()
        }
        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            val token = menuToken ?: return false
            if (!model.accepts(token)) return false
            if (item.itemId == android.R.id.copy) return copy(token).also { if (it) mode.finish() }
            val component = processorTargets[item.itemId] ?: return false
            val text = model.selectedText(token)?.takeIf { it.isNotEmpty() } ?: return false
            return try {
                callbacks.onSystemTextAction(Intent(Intent.ACTION_PROCESS_TEXT).setType("text/plain").setComponent(component)
                    .putExtra(Intent.EXTRA_PROCESS_TEXT, text).putExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, true))
                true
            } catch (_: android.content.ActivityNotFoundException) { false }
            catch (_: SecurityException) { false }
        }
        override fun onDestroyActionMode(mode: ActionMode) {
            actionMode = null; menuToken = null; processorTargets.clear()
            if (model.state.selection != null) model.cancel()
        }
        override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) {
            val value = current(); val point = model.state.selection?.focus
            val caret = if (value != null && point != null) geometry.caret(value, point) else null
            if (caret == null) outRect.set(0, 0, width, height) else outRect.set(
                (caret.x * density).toInt() + content.paddingLeft, (caret.top * density).toInt() + content.paddingTop,
                (caret.x * density).toInt() + content.paddingLeft + 1, (caret.bottom * density).toInt() + content.paddingTop)
        }
    }
    private val nodes = object : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(id: Int): AccessibilityNodeInfo? {
            val value = current() ?: return null
            if (id == View.NO_ID) return AccessibilityNodeInfo.obtain(this@TranscriptSelectionHost).apply {
                className = FrameLayout::class.java.name
                text = model.selectedText(model.token)
                value.frame.rowsIn(value.top, value.top + value.height).take(64).forEachIndexed { index, _ -> addChild(this@TranscriptSelectionHost, index + 1) }
            }
            val row = value.frame.rowsIn(value.top, value.top + value.height).take(64).getOrNull(id - 1) ?: return null
            val display = value.frame.display(row.index) ?: return null
            accessibleRows[id] = model.token to row.key
            return AccessibilityNodeInfo.obtain().apply {
                setSource(this@TranscriptSelectionHost, id); setParent(this@TranscriptSelectionHost)
                packageName = context.packageName; className = android.widget.TextView::class.java.name
                isVisibleToUser = true; isEnabled = true; isFocusable = true; isLongClickable = true
                isAccessibilityFocused = accessibilityFocus == row.key
                val range = model.state.rangeForRow(value.frame, row.key)
                text = range?.let { display.text.substring(it.first, it.second) } ?: display.text
                val bounds = Rect(content.paddingLeft, ((row.y - value.top) * density).toInt() + content.paddingTop,
                    width - content.paddingRight, ((row.y + row.height - value.top) * density).toInt() + content.paddingTop)
                setBoundsInParent(bounds)
                val location = IntArray(2)
                this@TranscriptSelectionHost.getLocationOnScreen(location)
                bounds.offset(location[0], location[1]); setBoundsInScreen(bounds)
                addAction(if (isAccessibilityFocused) AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS else AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS)
                addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION)
                addAction(AccessibilityNodeInfo.AccessibilityAction(AccessibilityNodeInfo.ACTION_LONG_CLICK, "Select text"))
                if (model.selectedText(model.token)?.isNotEmpty() == true) addAction(AccessibilityNodeInfo.ACTION_COPY)
            }
        }
        override fun performAction(id: Int, action: Int, arguments: Bundle?): Boolean {
            val (token, key) = accessibleRows[id] ?: return false
            if (!model.accepts(token)) return false
            return when (action) {
                AccessibilityNodeInfo.ACTION_LONG_CLICK -> model.selectRow(token, key).also { if (it) showActions() }
                AccessibilityNodeInfo.ACTION_COPY -> copy(token)
                AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> {
                    accessibilityFocus = key
                    accessibilityEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
                    true
                }
                AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS -> {
                    if (accessibilityFocus != key) false else {
                        accessibilityFocus = null
                        accessibilityEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
                        true
                    }
                }
                AccessibilityNodeInfo.ACTION_SET_SELECTION -> {
                    val start = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, -1) ?: -1
                    val end = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, -1) ?: -1
                    model.select(token, TranscriptTextPoint(key, start), TranscriptTextPoint(key, end))
                }
                else -> false
            }
        }
    }
    override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider = nodes
    fun cancelSelection() { gestureToken = null; draggingAnchor = null; model.cancel(); actionMode?.finish() }
    override fun onDetachedFromWindow() { close(); super.onDetachedFromWindow() }
    private fun retireBorrow() {
        if (!live) return
        live = false
        gestureToken = null; draggingAnchor = null
        try { model.close() } finally {
            actionMode?.finish()
            accessibleRows.clear()
            accessibilityFocus = null
        }
    }
    override fun close() {
        if (contentReleased) { retireBorrow(); return }
        contentReleased = true
        try { retireBorrow() } finally { (content as? AutoCloseable)?.close() }
    }
}
