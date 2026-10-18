// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Typeface
import android.os.Looper
import android.view.MotionEvent
import android.view.View
import uniffi.openagents_core.ControlKind
import uniffi.openagents_core.FaceRole
import uniffi.openagents_core.PointerPhase
import uniffi.openagents_core.Rgba
import uniffi.openagents_core.SurfaceFrame
import uniffi.openagents_core.SurfaceGesture
import uniffi.openagents_core.VisualControl
import uniffi.openagents_core.VisualDisplay
import uniffi.openagents_core.VisualListener
import uniffi.openagents_core.VisualPaint
import uniffi.openagents_core.VisualSession
import uniffi.openagents_core.VisualShape
import uniffi.openagents_core.VisualSurface

private const val MAX_IMAGE_SIDE = 8192
private const val MAX_IMAGE_BYTES = 16 * 1024 * 1024
private const val MAX_CACHED_BYTES = 32 * 1024 * 1024
private const val MAX_CACHED_SURFACES = 8
private const val MAX_PENDING_SURFACES = 16
private const val MAX_DISPLAY_TEXT = 512 * 1024
private const val MAX_PAINT_SHAPES = 4096
private const val MAX_PATH_POINTS = 32768

private fun isBoundedVisualPaint(paint: VisualPaint): Boolean {
    if (paint.text.length > MAX_DISPLAY_TEXT || paint.shapes.size > MAX_PAINT_SHAPES || paint.runs.size > 8192) return false
    if (paint.shapes.filterIsInstance<VisualShape.Path>().sumOf { it.points.size.toLong() } > MAX_PATH_POINTS) return false
    return paint.shapes.all { shape ->
        when (shape) {
            is VisualShape.Rect -> listOf(shape.x, shape.y, shape.w, shape.h, shape.radius).all(Float::isFinite)
            is VisualShape.Path -> shape.points.size <= MAX_PATH_POINTS && shape.points.size % 2 == 0 && shape.points.all(Float::isFinite)
        }
    }
}
internal fun isBoundedVisualDisplay(display: VisualDisplay): Boolean =
    display.width.isFinite() && display.height.isFinite() && display.width in 0f..16384f && display.height in 0f..16384f &&
        display.surfaces.size <= MAX_PENDING_SURFACES && display.controls.size <= 4096 && display.styles.size <= 8192 &&
        isBoundedVisualPaint(display.paint) && display.surfaces.all {
            listOf(it.x, it.y, it.w, it.h).all(Float::isFinite) && it.w in 0f..16384f && it.h in 0f..16384f
        }

internal fun isBoundedVisualFrame(frame: SurfaceFrame): Boolean {
    if (!isBoundedVisualPaint(frame.paint) || frame.styles.size > 8192) return false
    val image = frame.image ?: return true
    if (image.width == 0u || image.height == 0u || image.width > MAX_IMAGE_SIDE.toUInt() || image.height > MAX_IMAGE_SIDE.toUInt()) return false
    val pixels = image.width.toLong() * image.height.toLong()
    return pixels * 4 <= MAX_IMAGE_BYTES && image.bgra.size.toLong() == pixels * 4
}

/** Draws worker-owned Rust visual frames and forwards bounded pointer input. */
class AndroidVisualView(context: Context) : View(context) {
    private data class Raster(val frame: SurfaceFrame, val bitmap: Bitmap?, val bytes: Int)

    private val frames = LinkedHashMap<ULong, Raster>(8, 0.75f, true)
    private val delivery = VisualFrameMailbox<SurfaceFrame>(MAX_PENDING_SURFACES,
        post = { action -> post { action() } },
        receive = { epoch, frame ->
            if (deliveryIsCurrent(epoch)) { storeFrame(epoch, frame); invalidate() }
        },
        maxWeight = MAX_CACHED_BYTES.toLong(),
        weight = { frame -> (frame.image?.bgra?.size?.toLong() ?: 0L) + frame.paint.text.length * 2L + 2 * 1024 * 1024 },
    )
    private fun deliveryIsCurrent(epoch: Long): Boolean = delivery.isCurrent(epoch)
    private val typefaces = mutableMapOf<FaceRole, Typeface>()
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG)
    private val shapePaint = Paint(Paint.ANTI_ALIAS_FLAG)

    private var session: VisualSession? = null
    private var listener: VisualListener? = null
    private var listenerAttached = false
    private var display: VisualDisplay? = null
    private var dark = false
    private var reduceMotion = false
    private var layoutSize: Pair<Float, Float>? = null
    private var activeSurface: VisualSurface? = null
    private var activeControl: VisualControl? = null
    private var downX = 0f
    private var downY = 0f
    private var cancelledToParent = false

    init {
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES
    }

    /** Transfers one native handle; other views must use their own Rust handle. */
    fun bind(value: VisualSession, isDark: Boolean, reducedMotion: Boolean) {
        requireMainThread()
        try {
            delivery.reset()
            val previous = session
            try { detachListener() } finally {
                session = null
                display = null
                clearFrames()
                if (previous !== value) previous?.close()
            }
            session = value
            layoutSize = null
            dark = isDark
            reduceMotion = reducedMotion
            if (isAttachedToWindow) attachListener()
            invalidate()
            requestLayout()
        } catch (error: Throwable) {
            try { release() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            try { value.close() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        }
    }

    /** Requests a Rust layout in logical points. */
    fun layoutAt(width: Float, textScale: Float) {
        requireMainThread()
        if (!width.isFinite() || width !in 1f..16384f || !textScale.isFinite() || textScale !in 0.5f..4f) return
        val core = session ?: return
        val size = width to textScale
        if (layoutSize == size) return
        delivery.reset()
        try {
            detachListener()
            display = null
            clearFrames()
            core.layout(width, textScale)
            layoutSize = size
            if (isAttachedToWindow) attachListener()
        } catch (error: Throwable) {
            try { release() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        }
    }

    /** Releases this view's listener, bitmaps, and shared Rust handle. */
    fun release() {
        requireMainThread()
        delivery.reset()
        val previous = session
        try { detachListener() } finally {
            session = null
            listener = null
            display = null
            clearFrames()
            previous?.close()
        }
        invalidate()
        requestLayout()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        attachListener()
    }

    override fun onDetachedFromWindow() {
        delivery.reset()
        try { detachListener() } catch (error: Throwable) {
            try { release() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        } finally {
            display = null
            clearFrames()
            super.onDetachedFromWindow()
        }
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec)
        val points = display?.height?.takeIf { it.isFinite() && it > 0f } ?: 0f
        val height = (points * resources.displayMetrics.density).toInt().coerceAtMost(MAX_IMAGE_SIDE)
        setMeasuredDimension(width, resolveSize(height, heightMeasureSpec))
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val current = display ?: return
        val density = resources.displayMetrics.density
        canvas.save()
        canvas.scale(density, density)
        for (surface in current.surfaces.take(MAX_CACHED_SURFACES)) {
            val raster = frames[surface.id] ?: continue
            canvas.save()
            canvas.clipRect(surface.x, surface.y, surface.x + surface.w, surface.y + surface.h)
            raster.bitmap?.let { bitmap ->
                shapePaint.reset()
                shapePaint.isFilterBitmap = true
                canvas.drawBitmap(
                    bitmap,
                    null,
                    RectF(surface.x, surface.y, surface.x + surface.w, surface.y + surface.h),
                    shapePaint,
                )
            }
            canvas.translate(surface.x, surface.y)
            drawPaint(canvas, raster.frame.paint, raster.frame.styles.associateBy { it.id })
            canvas.restore()
        }
        drawPaint(canvas, current.paint, current.styles.associateBy { it.id })
        canvas.restore()
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val current = display ?: return false
        val density = resources.displayMetrics.density.coerceAtLeast(0.01f)
        val x = event.x / density
        val y = event.y / density
        val core = session ?: return false
        if (!x.isFinite() || !y.isFinite() || x !in -16384f..16384f || y !in -16384f..16384f) return false

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                cancelledToParent = false
                downX = x
                downY = y
                activeControl = current.controls.asReversed().firstOrNull { contains(it, x, y) }
                if (activeControl != null) {
                    parent?.requestDisallowInterceptTouchEvent(true)
                    activeControl?.let { updateControl(core, it, x) }
                    return true
                }
                activeSurface = current.surfaces.asReversed().firstOrNull {
                    it.gesture != SurfaceGesture.NONE && contains(it, x, y)
                }
                val surface = activeSurface ?: return false
                core.pointer(surface.id, PointerPhase.DOWN, x - surface.x, y - surface.y)
                parent?.requestDisallowInterceptTouchEvent(surface.gesture != SurfaceGesture.HORIZONTAL_DRAG)
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                activeControl?.let {
                    updateControl(core, it, x)
                    return true
                }
                val surface = activeSurface ?: return false
                if (surface.gesture == SurfaceGesture.HORIZONTAL_DRAG && !cancelledToParent) {
                    val dx = x - downX
                    val dy = y - downY
                    if (kotlin.math.abs(dy) > kotlin.math.abs(dx) && kotlin.math.abs(dy) > touchSlop()) {
                        core.pointer(surface.id, PointerPhase.CANCEL, x - surface.x, y - surface.y)
                        activeSurface = null
                        cancelledToParent = true
                        parent?.requestDisallowInterceptTouchEvent(false)
                        return false
                    }
                    if (kotlin.math.abs(dx) > touchSlop()) parent?.requestDisallowInterceptTouchEvent(true)
                }
                core.pointer(surface.id, PointerPhase.MOVE, x - surface.x, y - surface.y)
                return true
            }
            MotionEvent.ACTION_UP -> {
                activeControl?.let {
                    val stillOver = contains(it, x, y)
                    if (stillOver && it.kind !is ControlKind.Slider) core.activate(it.id)
                    activeControl = null
                    parent?.requestDisallowInterceptTouchEvent(false)
                    performClick()
                    return true
                }
                activeSurface?.let { core.pointer(it.id, PointerPhase.UP, x - it.x, y - it.y) }
                activeSurface = null
                parent?.requestDisallowInterceptTouchEvent(false)
                return !cancelledToParent
            }
            MotionEvent.ACTION_CANCEL -> {
                activeSurface?.let { core.pointer(it.id, PointerPhase.CANCEL, x - it.x, y - it.y) }
                activeSurface = null
                activeControl = null
                parent?.requestDisallowInterceptTouchEvent(false)
                return true
            }
        }
        return false
    }

    override fun performClick(): Boolean {
        super.performClick()
        return true
    }

    private fun attachListener() {
        val core = session ?: return
        if (listenerAttached) return
        val epoch = delivery.current()
        val callbacks = object : VisualListener {
            override fun display(value: VisualDisplay) {
                if (!delivery.isCurrent(epoch) || !isBoundedVisualDisplay(value)) return
                post {
                    if (!delivery.isCurrent(epoch)) return@post
                    val old = display
                    if (old != null && value.revision < old.revision) return@post
                    display = value
                    val visibleIds = value.surfaces.take(MAX_CACHED_SURFACES).mapTo(hashSetOf()) { it.id }
                    evictInvisible(visibleIds)
                    contentDescription = value.controls.asSequence().map { it.label }.take(16).joinToString(", ")
                    requestLayout()
                    invalidate()
                }
            }

            override fun frame(value: SurfaceFrame) {
                if (!delivery.isCurrent(epoch) || !validFrame(value)) return
                delivery.offer(epoch, value.surface, value)
            }
        }
        listener = callbacks
        listenerAttached = true
        try {
            core.attach(callbacks, dark, reduceMotion)
        } catch (error: Throwable) {
            try { release() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        }
    }

    private fun detachListener() {
        if (!listenerAttached) return
        listenerAttached = false
        val core = session
        listener = null
        core?.detach()
    }

    private fun storeFrame(epoch: Long, frame: SurfaceFrame) {
        if (!delivery.isCurrent(epoch)) return
        val bitmap = frame.image?.let { image ->
            visualRasterOrFallback(
                allocate = { Bitmap.createBitmap(image.width.toInt(), image.height.toInt(), Bitmap.Config.ARGB_8888) },
                copy = { it.copyPixelsFromBuffer(java.nio.ByteBuffer.wrap(image.bgra)) },
                dispose = { it.recycle() },
            )
        }
        if (!delivery.isCurrent(epoch)) { bitmap?.recycle(); return }
        frames.remove(frame.surface)?.bitmap?.recycle()
        val bytes = (bitmap?.byteCount ?: 0) + (frame.image?.bgra?.size ?: 0) + frame.paint.text.length * 2 +
            frame.paint.runs.size * 64 + frame.paint.shapes.size * 64 + frame.styles.size * 64 +
            frame.paint.shapes.filterIsInstance<VisualShape.Path>().sumOf { it.points.size * 4 }
        frames[frame.surface] = Raster(frame, bitmap, bytes)
        trimFrames()
    }

    private fun validFrame(frame: SurfaceFrame): Boolean {
        return isBoundedVisualFrame(frame)
    }

    private fun trimFrames() {
        var total = frames.values.sumOf { it.bytes }
        val iterator = frames.entries.iterator()
        while ((frames.size > MAX_CACHED_SURFACES || total > MAX_CACHED_BYTES) && iterator.hasNext()) {
            val entry = iterator.next()
            total -= entry.value.bytes
            entry.value.bitmap?.recycle()
            iterator.remove()
        }
    }

    private fun evictInvisible(visible: Set<ULong>) {
        val iterator = frames.entries.iterator()
        while (iterator.hasNext()) {
            val entry = iterator.next()
            if (entry.key !in visible) {
                entry.value.bitmap?.recycle()
                iterator.remove()
            }
        }
    }

    private fun clearFrames() {
        frames.values.forEach { it.bitmap?.recycle() }
        frames.clear()
        activeSurface = null
        activeControl = null
    }

    private fun drawPaint(canvas: Canvas, value: VisualPaint, styles: Map<UShort, uniffi.openagents_core.StyleDesc>) {
        if (value.shapes.size > MAX_PAINT_SHAPES || value.text.length > MAX_DISPLAY_TEXT) return
        value.shapes.forEach { shape ->
            when (shape) {
                is VisualShape.Rect -> {
                    if (!listOf(shape.x, shape.y, shape.w, shape.h, shape.radius).all(Float::isFinite)) return@forEach
                    shapePaint.reset()
                    shapePaint.isAntiAlias = true
                    shapePaint.color = shape.color.androidColor()
                    canvas.drawRoundRect(RectF(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h), shape.radius, shape.radius, shapePaint)
                }
                is VisualShape.Path -> {
                    if (shape.points.size < 4 || shape.points.size > MAX_PATH_POINTS || shape.points.size % 2 != 0) return@forEach
                    if (!shape.points.all(Float::isFinite)) return@forEach
                    val path = Path().apply {
                        moveTo(shape.points[0], shape.points[1])
                        var index = 2
                        while (index < shape.points.size) {
                            lineTo(shape.points[index], shape.points[index + 1])
                            index += 2
                        }
                        if (shape.closed) close()
                    }
                    shape.fill?.let {
                        shapePaint.reset()
                        shapePaint.isAntiAlias = true
                        shapePaint.style = Paint.Style.FILL
                        shapePaint.color = it.androidColor()
                        canvas.drawPath(path, shapePaint)
                    }
                    shape.stroke?.let {
                        shapePaint.reset()
                        shapePaint.isAntiAlias = true
                        shapePaint.style = Paint.Style.STROKE
                        shapePaint.strokeWidth = shape.lineWidth.takeIf { n -> n.isFinite() && n > 0f } ?: 1f
                        shapePaint.color = it.androidColor()
                        shape.dash?.takeIf { d -> d.isFinite() && d > 0f }?.let { d ->
                            shapePaint.pathEffect = DashPathEffect(floatArrayOf(d, d), 0f)
                        }
                        canvas.drawPath(path, shapePaint)
                    }
                }
            }
        }
        val utf16 = value.text
        value.runs.take(8192).forEach { run ->
            val start = run.start.toInt()
            val end = start + run.len.toInt()
            if (start < 0 || end > utf16.length || end < start || !run.x.isFinite() || !run.baseline.isFinite()) return@forEach
            val style = styles[run.style] ?: return@forEach
            textPaint.reset()
            textPaint.isAntiAlias = true
            textPaint.isSubpixelText = true
            textPaint.color = run.color.androidColor()
            textPaint.textSize = style.size.takeIf { it.isFinite() && it > 0f } ?: 14f
            textPaint.typeface = typeface(style.face)
            canvas.drawText(utf16, start, end, run.x, run.baseline, textPaint)
        }
    }

    private fun typeface(face: FaceRole): Typeface = typefaces.getOrPut(face) {
        val asset = when (face) {
            FaceRole.SANS -> "fonts/Geist.ttf"
            FaceRole.SANS_MEDIUM -> "fonts/Geist-Medium.ttf"
            FaceRole.SANS_SEMIBOLD -> "fonts/Geist-SemiBold.ttf"
            FaceRole.SANS_BOLD -> "fonts/Geist-Bold.ttf"
            FaceRole.SANS_ITALIC -> "fonts/Geist-Italic.ttf"
            FaceRole.SANS_MEDIUM_ITALIC -> "fonts/Geist-MediumItalic.ttf"
            FaceRole.SANS_SEMIBOLD_ITALIC -> "fonts/Geist-SemiBoldItalic.ttf"
            FaceRole.SANS_BOLD_ITALIC -> "fonts/Geist-BoldItalic.ttf"
            FaceRole.MONO -> "fonts/GeistMono.ttf"
            FaceRole.MONO_MEDIUM -> "fonts/GeistMono-Medium.ttf"
            FaceRole.MONO_SEMIBOLD -> "fonts/GeistMono-SemiBold.ttf"
            FaceRole.MONO_ITALIC -> "fonts/GeistMono-Italic.ttf"
        }
        runCatching { Typeface.createFromAsset(context.assets, asset) }
            .getOrElse { Typeface.create(if (face.name.startsWith("MONO")) "monospace" else "sans-serif", Typeface.NORMAL) }
    }

    private fun updateControl(core: VisualSession, control: VisualControl, x: Float) {
        val slider = control.kind as? ControlKind.Slider ?: return
        val fraction = boundedSliderFraction(x, slider.trackX, slider.trackW) ?: return
        core.slide(control.id, fraction)
    }

    private fun contains(control: VisualControl, x: Float, y: Float) =
        control.x.isFinite() && control.y.isFinite() && control.w.isFinite() && control.h.isFinite() &&
            control.w >= 0f && control.h >= 0f && x in control.x..(control.x + control.w) && y in control.y..(control.y + control.h)

    private fun contains(surface: VisualSurface, x: Float, y: Float) =
        surface.x.isFinite() && surface.y.isFinite() && surface.w.isFinite() && surface.h.isFinite() &&
            surface.w > 0f && surface.h > 0f && x in surface.x..(surface.x + surface.w) && y in surface.y..(surface.y + surface.h)

    private fun touchSlop() = 8f

    private fun Rgba.androidColor(): Int = android.graphics.Color.argb(
        (a.coerceIn(0f, 1f) * 255).toInt(),
        (r.coerceIn(0f, 1f) * 255).toInt(),
        (g.coerceIn(0f, 1f) * 255).toInt(),
        (b.coerceIn(0f, 1f) * 255).toInt(),
    )

    private fun requireMainThread() {
        check(Looper.myLooper() == Looper.getMainLooper()) { "Visual views must be updated on the main thread" }
    }
}

internal fun boundedSliderFraction(x: Float, trackX: Float, trackW: Float): Float? {
    if (!x.isFinite() || !trackX.isFinite() || !trackW.isFinite() || trackW <= 0f) return null
    val fraction = (x - trackX) / trackW
    return if (fraction.isFinite()) fraction.coerceIn(0f, 1f) else null
}
