// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.content.Context
import android.graphics.Canvas
import android.os.Looper
import android.view.MotionEvent
import android.view.VelocityTracker
import android.view.View
import android.view.ViewConfiguration
import android.widget.OverScroller
import java.lang.ref.WeakReference
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.abs
import uniffi.openagents_core.LayoutListener

data class TranscriptPaintStats(val revision: ULong, val visibleRows: Int, val cachedRows: Int, val drawNanos: Long)

/** One Android View paints only Rust's visible rows; child count never grows with history. */
class TranscriptCanvas(
    context: Context,
    fonts: TranscriptFonts,
    palette: TranscriptPalette,
    widgets: TranscriptWidgets? = null,
) : View(context), AutoCloseable {
    private val viewport = TranscriptViewport()
    private val painter = TranscriptPainter(fonts, palette, widgets)
    private val fling = OverScroller(context)
    private val config = ViewConfiguration.get(context)
    private val density get() = resources.displayMetrics.density
    private data class ScrollerKey(val row: ULong, val index: UInt)
    private val horizontal = object : LinkedHashMap<ScrollerKey, Float>(16, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<ScrollerKey, Float>?) = size > 96
    }
    private var pendingAnchor: TranscriptViewportAnchor? = null
    private var tracker: VelocityTracker? = null
    private var pointer = -1
    private var downX = 0f
    private var downY = 0f
    private var lastX = 0f
    private var lastY = 0f
    private var dragging = false
    private var horizontalDrag = false
    private var horizontalTarget: Pair<ScrollerKey, Float>? = null
    var onViewportChanged: ((width: Float, textScale: Float) -> Unit)? = null
    var onPaintStats: ((TranscriptPaintStats) -> Unit)? = null
    /** The shell closes its LayoutListener and Rust transcript here. */
    var onRelease: (() -> Unit)? = null
    var interactionObserver: TranscriptInteractionObserver? = null

    /** Borrowed immutable frame and copied presentation offsets; no second viewport. */
    fun interactionSnapshot(): TranscriptInteractionSnapshot? {
        check(Looper.myLooper() == Looper.getMainLooper())
        val frame = viewport.frame ?: return null
        val offsets = horizontal.toMap()
        return TranscriptInteractionSnapshot(frame, viewport.offset, viewport.height) { row, scroller ->
            offsets[ScrollerKey(row, scroller)] ?: 0f
        }
    }

    init { isVerticalScrollBarEnabled = true; isFocusable = true }

    fun submitFrame(frame: TranscriptFrame) {
        check(Looper.myLooper() == Looper.getMainLooper())
        val old = viewport.frame
        if (old === frame) return
        fling.abortAnimation()
        interactionObserver?.beforeFrameReplacement(frame)
        viewport.submit(frame)
        pendingAnchor?.let { if (viewport.restoreAnchor(it)) pendingAnchor = null }
        painter.setStyles(frame.styles())
        interactionObserver?.viewportChanged()
        (old as? AutoCloseable)?.close()
        invalidate()
    }

    fun saveViewportAnchor(): TranscriptViewportAnchor? {
        check(Looper.myLooper() == Looper.getMainLooper())
        return pendingAnchor ?: viewport.exportAnchor()
    }

    fun restoreViewportAnchor(anchor: TranscriptViewportAnchor) {
        check(Looper.myLooper() == Looper.getMainLooper())
        fling.abortAnimation()
        pendingAnchor = if (viewport.restoreAnchor(anchor)) null else anchor
        interactionObserver?.viewportChanged()
        invalidate()
    }

    fun followTail() { pendingAnchor = null; fling.abortAnimation(); viewport.followTail(); interactionObserver?.viewportChanged(); invalidate() }

    fun refreshViewport() {
        onViewportChanged?.invoke(((width - paddingLeft - paddingRight) / density).coerceAtLeast(0f), resources.configuration.fontScale)
        viewport.resize(((height - paddingTop - paddingBottom) / density).coerceAtLeast(0f))
        interactionObserver?.viewportChanged()
        invalidate()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) { refreshViewport() }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        refreshViewport()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val frame = viewport.frame ?: return
        val started = System.nanoTime()
        val rows = viewport.visibleRows()
        val save = canvas.save()
        canvas.clipRect(paddingLeft, paddingTop, width - paddingRight, height - paddingBottom)
        canvas.translate(paddingLeft.toFloat(), paddingTop.toFloat())
        canvas.scale(density, density)
        var revealing = false
        val reducedMotion = !android.animation.ValueAnimator.areAnimatorsEnabled()
        val nowMs = System.nanoTime() / 1_000_000L
        for (row in rows) {
            val sample = viewport.displaySample(row) ?: continue
            val display = sample.display
            val elapsed = (nowMs - sample.sampledAtMs).coerceIn(0L, UInt.MAX_VALUE.toLong())
            if (transcriptNeedsRevealFrame(display, elapsed, reducedMotion)) revealing = true
            val rowSave = canvas.save()
            canvas.translate(0f, row.y - viewport.offset)
            painter.draw(canvas, display, density, elapsed, reducedMotion) { horizontal[ScrollerKey(row.key, it)] ?: 0f }
            canvas.restoreToCount(rowSave)
        }
        canvas.restoreToCount(save)
        if (revealing && isAttachedToWindow) postInvalidateOnAnimation()
        onPaintStats?.invoke(TranscriptPaintStats(frame.revision, rows.size, viewport.cachedRows, System.nanoTime() - started))
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                pendingAnchor = null
                fling.abortAnimation()
                tracker?.recycle()
                tracker = VelocityTracker.obtain()
                tracker?.addMovement(event)
                pointer = event.getPointerId(0)
                downX = event.x; lastX = event.x
                downY = event.y; lastY = event.y
                dragging = false; horizontalDrag = false
                horizontalTarget = scrollerAt(event.x, event.y)
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                val index = event.findPointerIndex(pointer)
                if (index < 0) { endTouch(); return true }
                tracker?.addMovement(event)
                val x = event.getX(index); val y = event.getY(index)
                if (!dragging && maxOf(abs(x - downX), abs(y - downY)) > config.scaledTouchSlop) {
                    dragging = true
                    horizontalDrag = horizontalTarget != null && abs(x - downX) > abs(y - downY)
                    parent?.requestDisallowInterceptTouchEvent(true)
                }
                if (dragging) {
                    val target = horizontalTarget
                    if (horizontalDrag && target != null) {
                        horizontal[target.first] = ((horizontal[target.first] ?: 0f) + (lastX - x) / density).coerceIn(0f, target.second)
                    } else viewport.userScrollTo(viewport.offset + (lastY - y) / density, true)
                    interactionObserver?.viewportChanged()
                    invalidate()
                }
                lastX = x; lastY = y
                return true
            }
            MotionEvent.ACTION_UP -> {
                tracker?.addMovement(event)
                if (dragging && !horizontalDrag) {
                    tracker?.computeCurrentVelocity(1000, config.scaledMaximumFlingVelocity.toFloat())
                    val velocity = -(tracker?.getYVelocity(pointer) ?: 0f).toInt()
                    viewport.userScrollTo(viewport.offset, false)
                    if (abs(velocity) >= config.scaledMinimumFlingVelocity) {
                        fling.fling(0, (viewport.offset * density).toInt(), 0, velocity, 0, 0, 0, (viewport.maxOffset * density).toInt())
                        postInvalidateOnAnimation()
                    }
                } else if (!dragging) performClick()
                endTouch()
                return true
            }
            MotionEvent.ACTION_CANCEL -> { endTouch(); return true }
            MotionEvent.ACTION_POINTER_UP -> if (event.getPointerId(event.actionIndex) == pointer) { endTouch(); return true }
        }
        return pointer >= 0
    }

    private fun scrollerAt(x: Float, y: Float): Pair<ScrollerKey, Float>? {
        val frame = viewport.frame ?: return null
        val logicalY = (y - paddingTop) / density + viewport.offset
        val row = frame.indexAt(logicalY)?.let(frame::placement) ?: return null
        val display = viewport.display(row) ?: return null
        val localX = (x - paddingLeft) / density
        val localY = logicalY - row.y
        val index = display.scrollers.indexOfFirst { localX >= it.x && localX < it.x + it.w && localY >= it.y && localY < it.y + it.h && it.contentWidth > it.w }
        if (index < 0) return null
        return ScrollerKey(row.key, index.toUInt()) to (display.scrollers[index].contentWidth - display.scrollers[index].w)
    }

    private fun endTouch() {
        tracker?.recycle(); tracker = null; pointer = -1
        horizontalTarget = null; dragging = false
        parent?.requestDisallowInterceptTouchEvent(false)
    }

    override fun computeScroll() {
        if (fling.computeScrollOffset()) {
            viewport.userScrollTo(fling.currY / density, false)
            interactionObserver?.viewportChanged()
            postInvalidateOnAnimation()
        }
    }
    override fun performClick(): Boolean { super.performClick(); return true }
    override fun computeVerticalScrollRange() = ((viewport.frame?.totalHeight ?: 0f) * density).toInt().coerceAtLeast(height)
    override fun computeVerticalScrollOffset() = (viewport.offset * density).toInt()
    override fun computeVerticalScrollExtent() = (viewport.height * density).toInt()
    override fun onDetachedFromWindow() { close(); super.onDetachedFromWindow() }

    override fun close() {
        check(Looper.myLooper() == Looper.getMainLooper())
        fling.abortAnimation(); endTouch()
        val release = onRelease
        val observer = interactionObserver
        onRelease = null
        interactionObserver = null
        try { observer?.beforeFrameClose() } finally {
            try { release?.invoke() } finally {
                (viewport.frame as? AutoCloseable)?.close()
                viewport.clear(); painter.clear(); horizontal.clear(); pendingAnchor = null
                onViewportChanged = null; onPaintStats = null
            }
        }
    }
}

/** Coalesce worker notifications; fetch only the latest immutable frame on the UI thread. */
class TranscriptFrameRelay(view: TranscriptCanvas, latest: () -> TranscriptFrame) : LayoutListener, AutoCloseable {
    private val view = WeakReference(view)
    private val fetch = AtomicReference<(() -> TranscriptFrame)?>(latest)
    private val active = AtomicBoolean(true)
    private val queued = AtomicBoolean(false)
    override fun frameReady(revision: ULong) {
        if (!active.get() || !queued.compareAndSet(false, true)) return
        val target = view.get()
        if (target == null) { queued.set(false); return }
        if (!target.post {
            queued.set(false)
            if (active.get()) {
                val owner = view.get()
                val latestFrame = fetch.get()
                if (owner != null && latestFrame != null) owner.submitFrame(latestFrame())
            }
        }) queued.set(false)
    }
    override fun close() { active.set(false); fetch.set(null); view.clear() }
}
