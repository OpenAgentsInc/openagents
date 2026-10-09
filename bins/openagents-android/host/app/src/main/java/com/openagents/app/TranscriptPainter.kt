// The transcript painter. Rust lays out every row (crates/rust-native,
// `layout`): exact heights, offsets, and display lists with each text run's
// position, measured by Rust's shaper with the system fonts. This file only
// paints the runs at Rust's positions with the same fonts, scrolls, and
// handles taps. It mirrors bins/coder-ios/host/App/NativeTranscriptPainter.swift.
package com.openagents.app

import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Typeface
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.TextPaint
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.animation.AnimationUtils
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.TextView
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

/**
 * Transcript layout in `openagents-mobile` (`src/android/transcripts.rs`). A
 * transcript is updated on its own worker; frames are read on the UI thread
 * and released once. A failed call throws a RuntimeException.
 */
object TranscriptNative {
    init { System.loadLibrary("openagents_mobile") }
    @JvmStatic external fun create(): Long
    @JvmStatic external fun update(handle: Long, request: String): String
    @JvmStatic external fun destroy(handle: Long)
    @JvmStatic external fun frameRelease(frame: Long)
    @JvmStatic external fun frameHeight(frame: Long): Float
    /** Four values per row: index, version, and the float bits of its top and height. */
    @JvmStatic external fun frameAll(frame: Long): LongArray
    @JvmStatic external fun frameRows(frame: Long, y0: Float, y1: Float): LongArray
    /** Row keys joined by line feeds. */
    @JvmStatic external fun frameKeys(frame: Long): String
    @JvmStatic external fun frameDisplay(frame: Long, index: Int): String
    /** Face, `wght`, `opsz`, and `calt` (1 or 0). */
    @JvmStatic external fun fontSpec(size: Float, weight: Int, italic: Boolean, mono: Boolean): FloatArray
    @JvmStatic external fun fontData(face: Int): ByteArray
    /** Debug fixture only: publishes a transcript node's rows as the source `name`. */
    @JvmStatic external fun publish(name: String, node: String)
    /** Paint-only syntax spans for a code block, as JSON `[[start16, len16, [r, g, b, a]]]`. */
    @JvmStatic external fun highlight(language: String, text: String, light: Boolean): String
}

/** Rust's faces (the system's, `openagents-mobile` `android::typeface`) at the variations Rust measured with. */
internal object TranscriptFonts {
    private val typefaces = HashMap<String, Pair<Typeface, Boolean>>()
    private val files = HashMap<Int, File>()

    @Synchronized private fun file(context: Context, face: Int): File = files.getOrPut(face) {
        val data = TranscriptNative.fontData(face)
        val file = File(File(context.cacheDir, "transcript-fonts").apply { mkdirs() }, "face-$face.ttf")
        if (file.length() != data.size.toLong()) file.writeBytes(data)
        file
    }

    /** A typeface for Rust's font, and whether contextual alternates stay on. */
    @Synchronized fun of(context: Context, size: Float, weight: Int, italic: Boolean, mono: Boolean): Pair<Typeface, Boolean> {
        val key = "$size/$weight/$italic/$mono"
        return typefaces.getOrPut(key) {
            val spec = TranscriptNative.fontSpec(size, weight, italic, mono)
            val variations = buildString {
                append("'wght' ${spec[1]}")
                if (spec[2] > 0f) append(", 'opsz' ${spec[2]}")
            }
            val typeface = Typeface.Builder(file(context, spec[0].toInt()))
                .setFontVariationSettings(variations).build()
                ?: (Fonts.REGULAR + 100 * weight).let { if (mono) Fonts.code(context, it) else Fonts.typeface(context, it) }
            typeface to (spec[3] != 0f)
        }
    }
}

/** A row's display list (`rust_native::layout::display`), ready to paint. */
/**
 * Paint-only syntax colors for code blocks, from Rust Native's shared
 * highlighter. Highlighting runs on its own worker; a row paints plain code
 * until its spans arrive, then the transcripts listening repaint. Fonts,
 * text, and layout never change.
 */
internal object Syntax {
    class Span(val start: Int, val length: Int, val color: Int)
    private data class Key(val language: String, val text: String)

    private val cache = java.util.Collections.synchronizedMap(object : LinkedHashMap<Key, List<Span>>(32, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, List<Span>>?) = size > 64
    })
    private val pending = java.util.Collections.synchronizedSet(HashSet<Key>())
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "openagents-syntax") }
    private val main by lazy { Handler(Looper.getMainLooper()) }
    val listeners = java.util.concurrent.CopyOnWriteArrayList<() -> Unit>()

    /** The spans for `text`, or null while they are being worked out. Safe off the UI thread. */
    fun spans(language: String, text: String): List<Span>? {
        val key = Key(language, text)
        cache[key]?.let { return it }
        if (language.isEmpty() || text.isEmpty() || text.length > 65_536 || pending.size >= 16 || !pending.add(key)) return null
        worker.execute {
            val spans = runCatching { parse(TranscriptNative.highlight(language, text, false)) }.getOrDefault(emptyList())
            cache[key] = spans
            pending.remove(key)
            if (spans.isNotEmpty()) main.post { listeners.forEach { it() } }
        }
        return null
    }

    fun parse(json: String): List<Span> = JSONArray(json).let { rows ->
        (0 until rows.length()).mapNotNull { index ->
            val row = rows.optJSONArray(index) ?: return@mapNotNull null
            val rgba = row.optJSONArray(2) ?: return@mapNotNull null
            if (row.length() != 3 || rgba.length() != 4) return@mapNotNull null
            val (r, g, b, a) = List(4) { rgba.getInt(it).coerceIn(0, 255) }
            Span(row.getInt(0), row.getInt(1), (a shl 24) or (r shl 16) or (g shl 8) or b)
        }
    }
}

internal class RowModel(val json: JSONObject, private val context: Context) {
    class Style(val paint: TextPaint, val size: Float, val underline: Boolean, val strike: Boolean)
    class Run(val text: Int, val start: Int, val length: Int, val x: Float, val baseline: Float,
              val width: Float, val style: Int, val truncate: Float?)
    class Rect(val box: RectF, val radii: FloatArray, val fill: Int?, val stroke: Int?)
    class Widget(val box: RectF, val kind: String, val json: JSONObject)
    class Scroller(val box: RectF, val contentWidth: Float, val runs: IntRange, val rects: IntRange)

    /** Pixels per point. Text is drawn in pixels: a scaled canvas makes Android rasterize glyphs slowly. */
    private val pixels = context.resources.displayMetrics.density
    val key: String = json.getString("key")
    val height = json.getDouble("height").toFloat()
    private val texts = json.getJSONArray("texts").let { a -> List(a.length()) { a.getString(it) } }
    private val styles: List<Style> = json.getJSONArray("styles").objects().map { style(it) }
    private val runs = json.getJSONArray("runs").objects().map {
        Run(it.getInt("text"), it.getInt("start16"), it.getInt("len16"), it.f("x"), it.f("baseline"),
            it.f("width"), it.getInt("style"), if (it.has("truncate")) it.f("truncate") else null)
    }
    private val rects = json.getJSONArray("rects").objects().map { r ->
        val radii = r.getJSONArray("radii").let { a -> FloatArray(4) { a.getDouble(it).toFloat() } }
        Rect(RectF(r.f("x"), r.f("y"), r.f("x") + r.f("w"), r.f("y") + r.f("h")), radii,
            r.objectOrNull("fill")?.let { ink(it, 1f) }, r.objectOrNull("stroke")?.let { ink(it, 1f) })
    }
    val widgets = json.getJSONArray("widgets").objects().map {
        Widget(RectF(it.f("x"), it.f("y"), it.f("x") + it.f("w"), it.f("y") + it.f("h")), it.getString("kind"), it)
    }
    val scrollers = (json.optJSONArray("scrollers") ?: JSONArray()).objects().map {
        val r = it.getJSONArray("runs"); val q = it.getJSONArray("rects")
        Scroller(RectF(it.f("x"), it.f("y"), it.f("x") + it.f("w"), it.f("y") + it.f("h")), it.f("content_w"),
            r.getInt(0) until r.getInt(1), q.getInt(0) until q.getInt(1))
    }.filter { it.runs.last < runs.size && it.rects.last < rects.size && it.box.width() > 0 }
    /** Syntax spans for each code paragraph, once Rust has them. */
    private val code: Map<Int, List<Syntax.Span>> = (json.optJSONArray("code_blocks") ?: JSONArray()).objects()
        .mapNotNull { block ->
            val text = block.getInt("text")
            texts.getOrNull(text)?.let { value -> Syntax.spans(block.getString("language"), value)?.let { text to it } }
        }.toMap()
    private val runScroller = IntArray(runs.size) { -1 }
    private val rectScroller = IntArray(rects.size) { -1 }
    private val clipped = arrayOfNulls<CharSequence>(runs.size)
    val label: String = json.getJSONObject("accessibility").let { a ->
        listOf(a.optString("label"), a.optString("value")).filter { it.isNotEmpty() }.joinToString(", ")
    }
    val copy: String? = if (json.has("copy") && !json.isNull("copy")) json.getString("copy") else null

    init {
        scrollers.forEachIndexed { index, s ->
            for (run in s.runs) runScroller[run] = index
            for (rect in s.rects) rectScroller[rect] = index
        }
    }

    /**
     * The row's text for in-place selection. Caret stops come from the
     * bundled font's advances, scaled to Rust's width for each run; lines
     * span the font's ascent and descent.
     */
    val selectable: SelectableText by lazy {
        SelectableText(texts, runs.mapIndexed { index, run ->
            val style = styles.getOrNull(run.style) ?: return@mapIndexed null
            val text = texts.getOrNull(run.text) ?: return@mapIndexed null
            val end = run.start + run.length
            if (run.start < 0 || run.length <= 0 || end > text.length) return@mapIndexed null
            val metrics = style.paint.fontMetrics
            SelectableRun(run.text, run.start, run.length, run.x, run.baseline, run.width,
                -metrics.ascent / pixels, metrics.descent / pixels, runScroller[index], run.truncate) {
                val widths = FloatArray(run.length)
                style.paint.getTextWidths(text, run.start, end, widths)
                val carets = FloatArray(run.length + 1)
                for (i in 0 until run.length) carets[i + 1] = carets[i] + widths[i] / pixels
                carets
            }
        })
    }

    /** The runs and boxes as the streaming fade compares them (StreamFade.kt). */
    val fadeRuns: List<FadeRun> by lazy {
        runs.mapIndexed { index, run ->
            val text = texts.getOrNull(run.text) ?: ""
            val usable = run.start >= 0 && run.length > 0 && run.start + run.length <= text.length
            FadeRun(runScroller[index], run.style, run.x, run.baseline, run.width, text,
                if (usable) run.start else 0, if (usable) run.length else 0)
        }
    }
    val fadeRects: List<FadeRect> by lazy {
        rects.mapIndexed { index, rect ->
            FadeRect(rectScroller[index], rect.box.left, rect.box.top, rect.box.right, rect.fill, rect.stroke)
        }
    }

    private fun JSONObject.f(name: String) = getDouble(name).toFloat()

    private fun ink(ink: JSONObject, opacity: Float): Int {
        val base = ink.optJSONArray("rgba")?.let { Color.argb(it.getInt(3), it.getInt(0), it.getInt(1), it.getInt(2)) }
            ?: when (ink.optString("role")) {
                "secondary" -> Palette.SECONDARY
                "tertiary" -> Palette.TERTIARY
                "link" -> Palette.LINK
                "bubble" -> Palette.BUBBLE
                "surface" -> Palette.SURFACE
                "raised" -> Palette.RAISED
                "border" -> Palette.BORDER
                "inline_code" -> Palette.INLINE_CODE
                else -> Palette.PRIMARY
            }
        if (opacity >= 1f) return base
        return Color.argb((Color.alpha(base) * opacity).toInt(), Color.red(base), Color.green(base), Color.blue(base))
    }

    private fun style(style: JSONObject): Style {
        val font = style.getJSONObject("font")
        val size = font.getDouble("size").toFloat()
        val weight = when (font.getString("weight")) { "medium" -> 1; "semibold" -> 2; "bold" -> 3; else -> 0 }
        val (typeface, calt) = TranscriptFonts.of(context, size, weight, font.optBoolean("italic"), font.optBoolean("mono"))
        val paint = TextPaint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG).apply {
            this.typeface = typeface
            // TextPaint has its own `density` field, so name ours apart.
            textSize = size * pixels
            color = ink(style.getJSONObject("ink"), style.getDouble("opacity").toFloat())
            if (!calt) fontFeatureSettings = "'calt' 0"
        }
        return Style(paint, size, style.optBoolean("underline"), style.optBoolean("strike"))
    }

    /**
     * Paints the items of `scroller` (-1 for the row itself) at row
     * coordinates, which are points, into a canvas in pixels.
     */
    fun draw(canvas: Canvas, scroller: Int = -1, top: Float = Float.NEGATIVE_INFINITY, bottom: Float = Float.POSITIVE_INFINITY,
             highlights: List<TextBox>? = null, fade: RowFade? = null, now: Long = 0L) {
        val d = pixels
        val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        canvas.save()
        canvas.scale(d, d)
        rects.forEachIndexed { index, rect ->
            if (rectScroller[index] != scroller || rect.box.bottom < top || rect.box.top > bottom) return@forEachIndexed
            val alpha = fade?.rects?.getOrNull(index)?.let { fade.alpha(it, now) } ?: 1f
            val path = path(rect.box, rect.radii)
            rect.fill?.let { fill.style = Paint.Style.FILL; fill.color = faded(it, alpha); canvas.drawPath(path, fill) }
            rect.stroke?.let {
                fill.style = Paint.Style.STROKE; fill.strokeWidth = 1f; fill.color = faded(it, alpha)
                canvas.drawPath(path(RectF(rect.box).apply { inset(0.5f, 0.5f) }, rect.radii.map { r -> maxOf(0f, r - 0.5f) }.toFloatArray()), fill)
            }
        }
        if (highlights != null) {
            fill.style = Paint.Style.FILL; fill.color = Palette.SELECTION
            for (box in highlights) canvas.drawRect(box.left, box.top, box.right, box.bottom, fill)
        }
        canvas.restore()
        runs.forEachIndexed { index, run ->
            if (runScroller[index] != scroller || run.style >= styles.size || run.text >= texts.size) return@forEachIndexed
            val style = styles[run.style]
            if (run.baseline + style.size < top || run.baseline - style.size * 1.2f > bottom) return@forEachIndexed
            val text = texts[run.text]
            val end = run.start + run.length
            if (run.start < 0 || end > text.length || run.length <= 0) return@forEachIndexed
            val segments = fade?.runs?.getOrNull(index)
            if (segments == null) { drawRun(canvas, index, run, style, text); return@forEachIndexed }
            // New text fades in; the text already shown keeps full strength.
            // Each segment is clipped to its stretch of the run.
            val base = style.paint.alpha
            segments.forEachIndexed { i, segment ->
                val alpha = fade.alpha(segment.start, now)
                if (alpha <= 0f) return@forEachIndexed
                val left = if (i == 0) Float.NEGATIVE_INFINITY else (run.x + segment.from) * d
                val right = segments.getOrNull(i + 1)?.let { (run.x + it.from) * d } ?: Float.POSITIVE_INFINITY
                canvas.save()
                canvas.clipRect(maxOf(left, -1e7f), -1e7f, minOf(right, 1e7f), 1e7f)
                style.paint.alpha = Math.round(base * alpha)
                drawRun(canvas, index, run, style, text)
                style.paint.alpha = base
                canvas.restore()
            }
        }
    }

    private fun drawRun(canvas: Canvas, index: Int, run: Run, style: Style, text: String) {
        val d = pixels
        val end = run.start + run.length
        val limit = run.truncate
        if (limit != null) {
            val shown = clipped[index] ?: TextUtils.ellipsize(text.substring(run.start, end), style.paint,
                maxOf(0f, limit) * d, TextUtils.TruncateAt.END).also { clipped[index] = it }
            canvas.drawText(shown, 0, shown.length, run.x * d, run.baseline * d, style.paint)
        } else {
            val spans = code[run.text]
            if (spans.isNullOrEmpty()) canvas.drawText(text, run.start, end, run.x * d, run.baseline * d, style.paint)
            else drawColored(canvas, run, style, text, spans)
        }
        if (style.underline) canvas.drawRect(run.x * d, (run.baseline + 2f) * d, (run.x + run.width) * d, (run.baseline + 2f) * d + maxOf(1f, d), style.paint)
        if (style.strike) {
            val y = (run.baseline - style.size * 0.3f) * d
            canvas.drawRect(run.x * d, y, (run.x + run.width) * d, y + maxOf(1f, d), style.paint)
        }
    }

    /** A code run in its syntax colors: each stretch at the advance the paint gives it, so text never moves. */
    private fun drawColored(canvas: Canvas, run: Run, style: Style, text: String, spans: List<Syntax.Span>) {
        val d = pixels
        val end = run.start + run.length
        val paint = style.paint
        val base = paint.color
        var at = run.start
        fun draw(to: Int, color: Int) {
            if (to <= at) return
            paint.color = Color.argb(Color.alpha(color) * Color.alpha(base) / 255, Color.red(color), Color.green(color), Color.blue(color))
            val x = run.x * d + paint.measureText(text, run.start, at)
            canvas.drawText(text, at, to, x, run.baseline * d, paint)
            at = to
        }
        for (span in spans) {
            val from = maxOf(span.start, run.start)
            val to = minOf(span.start + span.length, end)
            if (to <= from || from < at) continue
            draw(from, base)
            draw(to, span.color)
        }
        draw(end, base)
        paint.color = base
    }

    private fun faded(color: Int, alpha: Float): Int =
        if (alpha >= 1f) color else Color.argb(Math.round(Color.alpha(color) * alpha), Color.red(color), Color.green(color), Color.blue(color))

    private fun path(box: RectF, radii: FloatArray): Path {
        val limit = minOf(box.width(), box.height()) / 2
        val r = FloatArray(4) { minOf(limit, maxOf(0f, radii.getOrElse(it) { 0f })) }
        return Path().apply {
            addRoundRect(box, floatArrayOf(r[0], r[0], r[1], r[1], r[2], r[2], r[3], r[3]), Path.Direction.CW)
        }
    }
}

/** One transcript row: its painting, widgets, sideways scrollers, and an in-place text selection. */
internal class RowView(context: Context) : FrameLayout(context) {
    var model: RowModel? = null; private set
    var toggle: (String) -> Unit = {}
    var loadEarlier: () -> Unit = {}
    /** Draws a surface the layout reserved a box for, such as a link card. */
    var surface: ((String) -> View?)? = null
    /** Called when this row's selection starts, changes, or ends, and when a scroller moves under it. */
    var selectionChanged: (RowView) -> Unit = {}
    /** The selected range, ordered, or null. */
    var selection: Pair<TextPosition, TextPosition>? = null; private set
    private var highlights: Map<Int, List<TextBox>> = emptyMap()
    private val scrollViews = ArrayList<HorizontalScrollView>()
    private val scrollContents = ArrayList<View>()
    private val density = context.resources.displayMetrics.density
    /** The layout epoch the model came from; a new width or text scale starts a new one. */
    private var epoch = 0
    /** New text that is fading in, or null. */
    private var fade: RowFade? = null

    init {
        setWillNotDraw(false)
        isFocusable = true
        setOnLongClickListener { offerCopy(); true }
    }

    fun bind(model: RowModel, epoch: Int) {
        if (this.model === model) return
        val old = this.model
        // New content in a row that keeps its layout, such as a streamed
        // reply, fades in; the text already shown does not fade again.
        val now = AnimationUtils.currentAnimationTimeMillis()
        fade = if (old != null && old.key == model.key && epoch == this.epoch) {
            StreamFade.carry(old.fadeRuns, old.fadeRects, fade?.takeIf { it.active(now) }, model.fadeRuns, model.fadeRects,
                now, fadeDuration())
        } else null
        this.epoch = epoch
        clearSelection()
        this.model = model
        removeAllViews()
        scrollViews.clear()
        scrollContents.clear()
        contentDescription = model.label
        for (scroller in model.scrollers.withIndex()) addView(scrollerView(model, scroller.index, scroller.value))
        for (widget in model.widgets) widgetView(widget)?.let { view ->
            addView(view, LayoutParams(px(widget.box.width()), px(widget.box.height())).apply {
                leftMargin = px(widget.box.left); topMargin = px(widget.box.top)
            })
        }
        invalidate()
    }

    private fun px(points: Float) = Math.round(points * density)

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val height = px(model?.height ?: 0f)
        super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY))
    }

    override fun onDraw(canvas: Canvas) {
        val now = AnimationUtils.currentAnimationTimeMillis()
        val fade = fade?.takeIf { it.active(now) }
        if (fade == null) this.fade = null
        model?.draw(canvas, highlights = highlights[-1], fade = fade, now = now)
        if (fade != null) {
            postInvalidateOnAnimation()
            scrollContents.forEach { it.postInvalidateOnAnimation() }
        }
    }

    /**
     * The fade's length under the system's animator duration scale; zero
     * when animations are off (Remove animations, or a scale of 0).
     */
    private fun fadeDuration(): Long {
        if (!ValueAnimator.areAnimatorsEnabled()) return 0L
        // Debug builds: `--ez rust_native_transcript_fade false` turns it off, to compare frame times.
        if (TranscriptDebug.ENABLED && (context as? android.app.Activity)?.intent
                ?.getBooleanExtra("rust_native_transcript_fade", true) == false) return 0L
        val scale = runCatching {
            Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
        }.getOrDefault(1f)
        return Math.round(StreamFade.DURATION_MS * scale.coerceIn(0f, 10f).toDouble())
    }

    private fun scrollerView(model: RowModel, index: Int, scroller: RowModel.Scroller): View {
        val content = object : View(context) {
            // A horizontal scroller measures its child's width unspecified.
            override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) =
                setMeasuredDimension(px(scroller.contentWidth), px(scroller.box.height()))

            override fun onDraw(canvas: Canvas) {
                canvas.save()
                canvas.translate(-scroller.box.left * density, -scroller.box.top * density)
                val now = AnimationUtils.currentAnimationTimeMillis()
                model.draw(canvas, index, highlights = highlights[index], fade = fade?.takeIf { it.active(now) }, now = now)
                canvas.restore()
            }
        }
        scrollContents.add(content)
        return HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = true
            overScrollMode = OVER_SCROLL_NEVER
            addView(content, LayoutParams(px(scroller.contentWidth), px(scroller.box.height())))
            layoutParams = LayoutParams(px(scroller.box.width()), px(scroller.box.height())).apply {
                leftMargin = px(scroller.box.left); topMargin = px(scroller.box.top)
            }
            setOnScrollChangeListener { _, _, _, _, _ -> if (selection != null) selectionChanged(this@RowView) }
            scrollViews.add(this)
        }
    }

    private fun widgetView(widget: RowModel.Widget): View? = when (widget.kind) {
        "copy" -> TextView(context).apply {
            text = "Copy"; textSize = 12f; setTextColor(Palette.SECONDARY); gravity = Gravity.CENTER_VERTICAL or Gravity.END
            contentDescription = "Copy code"
            val code = widget.json.optString("text")
            setOnClickListener {
                clipboard(code)
                text = "Copied"
                postDelayed({ text = "Copy" }, 1_500)
            }
        }
        "toggle" -> View(context).apply {
            val key = widget.json.optString("key")
            importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
            setOnClickListener { toggle(key) }
        }
        "chevron" -> Glyph(context, if (widget.json.optBoolean("expanded")) Glyph.Shape.DOWN else Glyph.Shape.RIGHT, Palette.TERTIARY)
        "status" -> when (widget.json.optString("state")) {
            "running" -> ProgressBar(context).apply { isIndeterminate = true; scaleX = 0.75f; scaleY = 0.75f }
            "failed" -> Glyph(context, Glyph.Shape.CROSS, Palette.FAILURE)
            else -> Glyph(context, Glyph.Shape.CHECK, Palette.SUCCESS)
        }
        "checkbox" -> Glyph(context, if (widget.json.optBoolean("checked")) Glyph.Shape.CHECKED else Glyph.Shape.BOX,
            if (widget.json.optBoolean("checked")) Palette.SUCCESS else Palette.SECONDARY)
        "working" -> WorkingDots(context)
        "spinner" -> ProgressBar(context).apply { isIndeterminate = true; scaleX = 0.8f; scaleY = 0.8f }
        // A link's text is already painted; this is its tap target. Rust admits
        // only `https` URLs (`rust_native::markdown::opens`), checked again here.
        "link" -> Uri.parse(widget.json.optString("url")).takeIf { it.scheme.equals("https", ignoreCase = true) }?.let { url ->
            View(context).apply {
                importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
                setOnClickListener {
                    val open = Intent(Intent.ACTION_VIEW, url)
                        .addCategory(Intent.CATEGORY_BROWSABLE)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    // With no browser installed, the tap does nothing.
                    try { context.startActivity(open) } catch (ignored: ActivityNotFoundException) {}
                }
            }
        }
        "earlier" -> if (widget.json.optBoolean("loading")) null else View(context).apply {
            importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
            setOnClickListener { loadEarlier() }
        }
        "surface" -> surface?.invoke(widget.json.optString("resource"))?.also { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        else -> null
    }

    override fun performClick(): Boolean {
        model?.widgets?.firstOrNull { it.kind == "earlier" && !it.json.optBoolean("loading") }?.let { loadEarlier() }
        return super.performClick()
    }

    // A long press on a message offers Copy and Select Text. Select Text
    // selects the painted text in place, with handles to adjust it.
    private fun offerCopy() {
        val model = model ?: return
        val text = model.copy ?: model.label
        if (text.isEmpty()) return
        val menu = PopupMenu(context, this)
        menu.menu.add(0, 0, 0, "Copy")
        if (model.selectable.hasText) menu.menu.add(0, 1, 1, "Select Text")
        val feedback = SelectionLayer.giveFeedback
        if (feedback != null && text.isNotBlank()) menu.menu.add(0, 2, 2, "Give feedback")
        menu.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                0 -> clipboard(text)
                2 -> feedback?.invoke(text, model.key)
                else -> post { selectAll() }
            }
            true
        }
        menu.show()
    }

    // Selection

    fun selectAll() {
        val bounds = model?.selectable?.textBounds() ?: return
        select(bounds.first, bounds.second)
    }

    fun select(start: TextPosition, end: TextPosition) {
        val model = model ?: return
        selection = if (start <= end) start to end else end to start
        highlights = model.selectable.highlights(selection!!.first, selection!!.second)
        repaint()
        selectionChanged(this)
    }

    fun clearSelection() {
        if (selection == null) return
        selection = null
        highlights = emptyMap()
        repaint()
        selectionChanged(this)
    }

    fun selectedText(): String {
        val (start, end) = selection ?: return ""
        return model?.selectable?.text(start, end) ?: ""
    }

    private fun repaint() {
        invalidate()
        scrollContents.forEach { it.invalidate() }
    }

    /** How far a scroller has moved sideways, in points. */
    fun scrollOffset(scroller: Int): Float = scrollViews.getOrNull(scroller)?.let { it.scrollX / density } ?: 0f

    /** A caret on screen, in this view's points: scrolled, and held inside its scroller. */
    fun caret(position: TextPosition): Caret? {
        val model = model ?: return null
        val caret = model.selectable.caret(position) ?: return null
        val scroller = model.selectable.scroller(position.run)
        var x = caret.x - scrollOffset(scroller)
        model.scrollers.getOrNull(scroller)?.let { x = x.coerceIn(it.box.left, it.box.right) }
        return Caret(x, caret.top, caret.bottom)
    }

    /** The selection's highlight on screen, in this view's points. */
    fun selectionBounds(): TextBox? {
        var box: TextBox? = null
        for ((scroller, boxes) in highlights) {
            val offset = scrollOffset(scroller)
            val clip = model?.scrollers?.getOrNull(scroller)?.box
            for (b in boxes) {
                var left = b.left - offset; var right = b.right - offset
                if (clip != null) { left = left.coerceIn(clip.left, clip.right); right = right.coerceIn(clip.left, clip.right) }
                box = box?.let { TextBox(minOf(it.left, left), minOf(it.top, b.top), maxOf(it.right, right), maxOf(it.bottom, b.bottom)) }
                    ?: TextBox(left, b.top, right, b.bottom)
            }
        }
        return box
    }

    /** The text position under a point in this view, in points. */
    fun position(x: Float, y: Float): TextPosition? {
        val model = model ?: return null
        for ((index, scroller) in model.scrollers.withIndex()) {
            if (scroller.box.contains(x, y)) {
                model.selectable.position(x + scrollOffset(index), y, index)?.let { return it }
            }
        }
        return model.selectable.position(x, y, -1)
    }

    fun clipboard(text: String) {
        (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
            .setPrimaryClip(ClipData.newPlainText("Message", text))
    }
}

/** A small drawn mark: a check or cross in a circle, a checkbox, or a chevron. */
internal class Glyph(context: Context, private val shape: Shape, private val tint: Int) : View(context) {
    enum class Shape { CHECK, CROSS, CHECKED, BOX, RIGHT, DOWN }
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = tint; strokeCap = Paint.Cap.ROUND }

    init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }

    override fun onDraw(canvas: Canvas) {
        val d = resources.displayMetrics.density
        val cx = width / 2f; val cy = height / 2f; val r = 7.5f * d
        val mark = Paint(paint).apply { style = Paint.Style.STROKE; strokeWidth = 1.8f * d }
        when (shape) {
            Shape.CHECK, Shape.CROSS -> {
                canvas.drawCircle(cx, cy, r, paint)
                mark.color = Palette.BACKGROUND
                if (shape == Shape.CHECK) {
                    canvas.drawPath(Path().apply { moveTo(cx - 3.5f * d, cy); lineTo(cx - 1f * d, cy + 2.5f * d); lineTo(cx + 3.5f * d, cy - 2.5f * d) }, mark)
                } else {
                    canvas.drawLine(cx - 3f * d, cy - 3f * d, cx + 3f * d, cy + 3f * d, mark)
                    canvas.drawLine(cx + 3f * d, cy - 3f * d, cx - 3f * d, cy + 3f * d, mark)
                }
            }
            Shape.CHECKED, Shape.BOX -> {
                val box = RectF(cx - r + d, cy - r + d, cx + r - d, cy + r - d)
                if (shape == Shape.CHECKED) {
                    canvas.drawRoundRect(box, 3 * d, 3 * d, paint)
                    mark.color = Palette.BACKGROUND
                    canvas.drawPath(Path().apply { moveTo(cx - 3.5f * d, cy); lineTo(cx - 1f * d, cy + 2.5f * d); lineTo(cx + 3.5f * d, cy - 2.5f * d) }, mark)
                } else {
                    mark.color = tint; mark.strokeWidth = 1.2f * d
                    canvas.drawRoundRect(box, 3 * d, 3 * d, mark)
                }
            }
            Shape.RIGHT -> canvas.drawPath(Path().apply { moveTo(cx - 2f * d, cy - 4f * d); lineTo(cx + 2f * d, cy); lineTo(cx - 2f * d, cy + 4f * d) }, mark)
            Shape.DOWN -> canvas.drawPath(Path().apply { moveTo(cx - 4f * d, cy - 2f * d); lineTo(cx, cy + 2f * d); lineTo(cx + 4f * d, cy - 2f * d) }, mark)
        }
    }
}

/** The animated working indicator: three pulsing dots. */
internal class WorkingDots(context: Context) : View(context) {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.SECONDARY }
    private val animator = ValueAnimator.ofFloat(0f, 1f).apply {
        duration = 1_200; repeatCount = ValueAnimator.INFINITE
        addUpdateListener { invalidate() }
    }

    init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }

    override fun onAttachedToWindow() { super.onAttachedToWindow(); animator.start() }
    override fun onDetachedFromWindow() { animator.cancel(); super.onDetachedFromWindow() }

    override fun onDraw(canvas: Canvas) {
        val d = resources.displayMetrics.density
        val t = animator.animatedValue as Float
        for (index in 0 until 3) {
            val phase = ((t - index * 0.17f) % 1f + 1f) % 1f
            paint.alpha = (255 * (0.25f + 0.75f * (1 - kotlin.math.abs(phase * 2 - 1)))).toInt()
            canvas.drawCircle(3 * d + index * 10 * d, height / 2f, 3 * d, paint)
        }
    }
}

/**
 * A bottom-anchored conversation painted from Rust's layout. Rust lays out
 * on a worker; the list shows each finished frame's rows at their exact
 * heights, so following the newest row, jumping to the bottom, and keeping
 * the reader's place when rows arrive above need no estimates. With a
 * transcript source (`rust_native::layout::source`), the rows never cross
 * the view: the update names the source, and Rust reads them.
 */
class RustTranscript(private val context: Context, private val surfaces: ((String) -> View?)? = null,
                     private val activate: (String) -> Unit) {
    private data class Item(val key: String, val version: Long, val index: Int, val height: Float, val top: Float)

    val root = FrameLayout(context)
    private val list = RecyclerView(context)
    private val layout = LinearLayoutManager(context)
    private val jump = context.text("↓", 18f).apply {
        gravity = Gravity.CENTER
        background = context.rounded(Palette.RAISED, 20f, Palette.BORDER)
        elevation = context.dpf(6f)
        contentDescription = "Scroll to bottom"
        tag = "transcript-scroll-to-bottom"
        visibility = View.GONE
    }
    private val main = Handler(Looper.getMainLooper())
    private val density = context.resources.displayMetrics.density
    private var handle = 0L
    private var items: List<Item> = emptyList()
    private var frame = 0L
    /** Row models by key and version; the prefetcher fills it off the UI thread. */
    private val models = java.util.Collections.synchronizedMap(object : LinkedHashMap<String, RowModel>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, RowModel>?) = size > 240
    })
    private var prefetched = -1
    private var key = ""
    private var source: String? = null
    private var children: JSONArray? = null
    private var earlier: JSONObject? = null
    private val expanded = HashSet<String>()
    private var following = true
    private var dirty = true
    private var sentWidth = 0f
    private var sentScale = 0f
    /** Counts layouts at a new width or text scale; rows fade new text only within one. */
    private var epoch = 0
    /** The epoch of the frame on screen. */
    private var frameEpoch = 0
    private var inFlight = false
    private var pending = false
    private var disposed = false
    /** Debug timings: the slowest update on the worker and the slowest bind, in nanoseconds. */
    private var worstUpdate = 0L
    private var worstBind = 0L
    private var bench: TranscriptBench? = null
    private var selectedOnce = false

    private val adapter = object : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
        override fun getItemCount() = items.size
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder =
            object : RecyclerView.ViewHolder(RowView(parent.context).apply {
                layoutParams = RecyclerView.LayoutParams(-1, -2)
            }) {}
        override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
            val view = holder.itemView as RowView
            val item = items[position]
            val started = System.nanoTime()
            view.surface = surfaces
            model(item)?.let { view.bind(it, frameEpoch) }
            worstBind = maxOf(worstBind, System.nanoTime() - started)
            view.toggle = { key -> if (!expanded.remove(key)) expanded.add(key); dirty = true; sync() }
            view.loadEarlier = { if (earlier?.optBoolean("loading") == false && key.isNotEmpty()) activate(key) }
            view.selectionChanged = { row -> selection.changed(row) }
        }
    }
    private lateinit var selection: SelectionLayer
    /** Code colors arrived: rebuild the rows' models and paint them again. */
    private val syntaxReady: () -> Unit = {
        if (!disposed) { models.clear(); adapter.notifyItemRangeChanged(0, items.size) }
    }

    init {
        Syntax.listeners.add(syntaxReady)
        list.layoutManager = layout
        list.adapter = adapter
        list.itemAnimator = null
        list.clipToPadding = false
        list.overScrollMode = View.OVER_SCROLL_NEVER
        list.addItemDecoration(object : RecyclerView.ItemDecoration() {
            override fun getItemOffsets(outRect: android.graphics.Rect, view: View, parent: RecyclerView, state: RecyclerView.State) {
                val position = parent.getChildAdapterPosition(view)
                outRect.top = Math.round((if (position == 0) EDGE else GAP) * density)
                outRect.bottom = if (position == items.size - 1) Math.round(EDGE * density) else 0
            }
        })
        list.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrollStateChanged(view: RecyclerView, state: Int) {
                when (state) {
                    RecyclerView.SCROLL_STATE_DRAGGING -> following = false
                    RecyclerView.SCROLL_STATE_IDLE -> if (!following) following = distanceFromBottom() < FOLLOW * density
                }
                if (following) pin()
                updateJump()
                selection.scrolling(state != RecyclerView.SCROLL_STATE_IDLE)
            }
            override fun onScrolled(view: RecyclerView, dx: Int, dy: Int) {
                updateJump()
                prefetch()
                selection.place()
            }
        })
        // A tap anywhere in the list ends a selection; the handles keep their drags.
        val taps = android.view.GestureDetector(context, object : android.view.GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapUp(e: android.view.MotionEvent): Boolean { selection.clear(); return false }
        })
        list.addOnItemTouchListener(object : RecyclerView.SimpleOnItemTouchListener() {
            override fun onInterceptTouchEvent(view: RecyclerView, e: android.view.MotionEvent): Boolean {
                if (selection.row != null) taps.onTouchEvent(e)
                return false
            }
        })
        list.addOnLayoutChangeListener { _, left, _, right, _, oldLeft, _, oldRight, _ ->
            if (right - left != oldRight - oldLeft) sync()
            if (following) pin()
        }
        root.addView(list, FrameLayout.LayoutParams(-1, -1))
        root.addView(jump, FrameLayout.LayoutParams(context.dp(40), context.dp(40)).apply {
            gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL; bottomMargin = context.dp(12)
        })
        jump.setOnClickListener { following = true; pin(); updateJump() }
        selection = SelectionLayer(root, list)
        // The layout lives while the view is attached; reattaching lays out
        // again from the current revision.
        root.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(view: View) {
                if (handle != 0L) return
                handle = runCatching { TranscriptNative.create() }.getOrDefault(0L)
                disposed = false
                dirty = true
                sync()
            }
            override fun onViewDetachedFromWindow(view: View) = dispose()
        })
    }

    /** Takes a new revision: a transcript source's name, or the rows themselves. */
    fun update(props: JSONObject) {
        list.contentDescription = props.getString("label")
        key = root.getTag(R.id.native_key) as? String ?: key
        earlier = props.objectOrNull("earlier")
        val named = if (props.has("source") && !props.isNull("source")) props.getString("source") else null
        source = named
        children = if (named == null) props.optJSONArray("children") else null
        dirty = true
        sync()
    }

    private val scale get() = context.resources.configuration.fontScale.coerceIn(0.5f, 4f)

    /** Sends Rust what changed; the finished frame replaces the one on screen. */
    private fun sync() {
        if (handle == 0L || disposed) return
        val handle = handle
        val width = list.width / density
        if (width <= 0f) return
        if (!dirty && width == sentWidth) return
        if (inFlight) { pending = true; return }
        val request = JSONObject().apply {
            put("width", width.toDouble())
            put("scale", scale.toDouble())
            put("expanded", JSONArray(expanded.toList()))
            val named = source
            if (named != null) put("source", named) else {
                val rows = children ?: JSONArray()
                put("order", JSONArray((0 until rows.length()).map { rows.getJSONObject(it).getString("key") }))
                put("rows", rows)
                earlier?.let { put("earlier", JSONObject().put("label", it.getString("label")).put("loading", it.optBoolean("loading"))) }
            }
        }.toString()
        dirty = false
        if (width != sentWidth || scale != sentScale) epoch += 1
        val requestEpoch = epoch
        sentWidth = width
        sentScale = scale
        inFlight = true
        val old = items
        worker.execute {
            val started = System.nanoTime()
            val outcome = runCatching {
                val reply = JSONObject(TranscriptNative.update(handle, request))
                if (reply.has("error")) null else {
                    val next = reply.getLong("frame")
                    val keys = TranscriptNative.frameKeys(next).let { if (it.isEmpty()) emptyList() else it.split('\n') }
                    val all = TranscriptNative.frameAll(next)
                    val fresh = List(keys.size) { i ->
                        Item(keys[i], all[4 * i + 1], all[4 * i].toInt(),
                            Float.fromBits(all[4 * i + 3].toInt()), Float.fromBits(all[4 * i + 2].toInt()))
                    }
                    val diff = DiffUtil.calculateDiff(object : DiffUtil.Callback() {
                        override fun getOldListSize() = old.size
                        override fun getNewListSize() = fresh.size
                        override fun areItemsTheSame(a: Int, b: Int) = old[a].key == fresh[b].key
                        override fun areContentsTheSame(a: Int, b: Int) = old[a].version == fresh[b].version
                    }, false)
                    Triple(next, fresh, diff)
                }
            }.getOrNull()
            val spent = System.nanoTime() - started
            main.post {
                worstUpdate = maxOf(worstUpdate, spent)
                inFlight = false
                if (disposed) { outcome?.let { TranscriptNative.frameRelease(it.first) }; return@post }
                if (outcome != null) present(outcome.first, outcome.second, outcome.third, requestEpoch)
                // A refusal waits for the next revision; it does not loop.
                if (pending) { pending = false; sync() }
            }
        }
    }

    private fun present(next: Long, fresh: List<Item>, diff: DiffUtil.DiffResult, epoch: Int) {
        val anchor = if (following) null else anchor()
        val previous = frame
        frame = next
        frameEpoch = epoch
        items = fresh
        diff.dispatchUpdatesTo(adapter)
        if (previous != 0L) TranscriptNative.frameRelease(previous)
        prefetched = -1
        prefetch()
        if (TranscriptDebug.ENABLED && bench == null && fresh.isNotEmpty() &&
            (context as? android.app.Activity)?.intent?.getBooleanExtra("rust_native_transcript_bench", false) == true) {
            worstBind = 0
            bench = TranscriptBench(list, { following = false; updateJump() }) {
                String.format(java.util.Locale.ROOT, "worst update %.2f ms, worst bind %.2f ms",
                    worstUpdate / 1e6, worstBind / 1e6)
            }.also { it.start() }
        }
        if (following) pin()
        else if (anchor != null) {
            val index = fresh.indexOfFirst { it.key == anchor.first }
            if (index >= 0) layout.scrollToPositionWithOffset(index, anchor.second)
        }
        updateJump()
        list.post { selection.place() }
        if (TranscriptDebug.ENABLED && !selectedOnce && fresh.isNotEmpty()) {
            (context as? android.app.Activity)?.intent?.getStringExtra("rust_native_transcript_select")?.let { wanted ->
                // Select a row's text in place, for screenshots.
                selectedOnce = true
                val index = fresh.indexOfFirst { it.key == wanted }
                if (index >= 0) {
                    following = false
                    layout.scrollToPositionWithOffset(index, context.dp(120))
                    list.postDelayed({
                        for (i in 0 until list.childCount) {
                            val row = list.getChildAt(i) as? RowView ?: continue
                            if (row.model?.key == wanted) row.selectAll()
                        }
                    }, 1_000)
                }
            }
        }
    }

    /** The first visible row's key and its view's top on screen. */
    private fun anchor(): Pair<String, Int>? {
        val position = layout.findFirstVisibleItemPosition()
        if (position == RecyclerView.NO_POSITION || position >= items.size) return null
        val view = layout.findViewByPosition(position) ?: return null
        return items[position].key to (view.top - list.paddingTop)
    }

    private fun model(item: Item): RowModel? {
        models["${item.key}/${item.version}"]?.let { return it }
        return build(frame, item)
    }

    /** A row's model from `frame`, kept for later binds. Safe off the UI thread. */
    private fun build(frame: Long, item: Item): RowModel? {
        if (frame == 0L) return null
        val display = runCatching { TranscriptNative.frameDisplay(frame, item.index) }.getOrNull() ?: return null
        return RowModel(JSONObject(display), context).also { models["${item.key}/${item.version}"] = it }
    }

    /**
     * Builds the models of rows near the screen on a background thread, so
     * binding a row rarely parses its display list on the UI thread.
     */
    private fun prefetch() {
        val first = layout.findFirstVisibleItemPosition()
        if (first == RecyclerView.NO_POSITION || kotlin.math.abs(first - prefetched) < 4) return
        prefetched = first
        val last = maxOf(first, layout.findLastVisibleItemPosition())
        val wanted = items.subList(maxOf(0, first - PREFETCH), minOf(items.size, last + PREFETCH + 1))
            .filter { !models.containsKey("${it.key}/${it.version}") }
        if (wanted.isEmpty()) return
        val frame = frame
        prefetcher.execute { for (item in wanted) if (!models.containsKey("${item.key}/${item.version}")) build(frame, item) }
    }

    /**
     * Room under the last row for what floats over the transcript's bottom
     * (the composer, #11126), in pixels; the scroll-to-bottom button sits
     * above it.
     */
    fun setBottomInset(pixels: Int) {
        if (list.paddingBottom == pixels) return
        list.setPadding(0, 0, 0, pixels)
        (jump.layoutParams as? FrameLayout.LayoutParams)?.let {
            it.bottomMargin = pixels + context.dp(12)
            jump.layoutParams = it
        }
        if (following) pin()
    }

    private fun distanceFromBottom() =
        list.computeVerticalScrollRange() - list.computeVerticalScrollOffset() - list.computeVerticalScrollExtent()

    private fun pin() {
        if (items.isEmpty() || list.scrollState != RecyclerView.SCROLL_STATE_IDLE) return
        list.post {
            if (!following || items.isEmpty()) return@post
            // Heights are exact, so the last row's position is the bottom.
            layout.scrollToPosition(items.size - 1)
            list.post { if (following && list.canScrollVertically(1)) list.scrollBy(0, distanceFromBottom()) }
        }
    }

    private fun updateJump() {
        val show = !following && list.height > context.dp(120) && distanceFromBottom() > context.dp(80)
        val visible = if (show) View.VISIBLE else View.GONE
        if (jump.visibility != visible) {
            jump.visibility = visible
            if (show) ObjectAnimator.ofFloat(jump, "alpha", 0f, 1f).setDuration(150).start()
        }
    }

    private fun dispose() {
        Syntax.listeners.remove(syntaxReady)
        selection.clear()
        if (disposed || handle == 0L) return
        disposed = true
        val (h, f) = handle to frame
        handle = 0L
        frame = 0L
        items = emptyList()
        adapter.notifyDataSetChanged()
        worker.execute {
            if (f != 0L) TranscriptNative.frameRelease(f)
            if (h != 0L) TranscriptNative.destroy(h)
        }
    }

    private companion object {
        /** Matches `rust_native::layout::EDGE_INSET` and `ROW_GAP`. */
        const val EDGE = 16f
        const val GAP = 18f
        /** Coming to rest this close to the bottom resumes following, in points. */
        const val FOLLOW = 70f
        val worker = Executors.newSingleThreadExecutor { Thread(it, "openagents-transcript-layout") }
        val prefetcher = Executors.newSingleThreadExecutor { Thread(it, "openagents-transcript-rows") }
        /** Rows prepared beyond each edge of the screen. */
        const val PREFETCH = 12
    }
}

/**
 * The transcript's fixture, benchmark, and selection launch extras exist
 * only in debug builds and in the separate benchmark app
 * (`scripts/build-openagents-android.sh bench`), never in the released app.
 */
internal object TranscriptDebug {
    val ENABLED: Boolean get() = BuildConfig.TRANSCRIPT_DEBUG
}

/** Debug and benchmark builds only: synthetic rows for the fixture, to check scrolling over a long transcript. */
internal object TranscriptFixture {
    /**
     * `view` with `count` synthetic rows before its transcript's own; with
     * `pull`, the rows are published to a Rust transcript source and the
     * transcript names it, as the app's chats do.
     */
    fun prepare(view: JSONObject, count: Int, pull: Boolean): JSONObject {
        withRows(view, count)
        if (!pull) return view
        fun find(node: JSONObject): JSONObject? {
            val element = node.getJSONObject("element")
            if (element.getString("kind") == "transcript") return node
            val children = element.optJSONObject("props")?.optJSONArray("children") ?: return null
            return children.objects().firstNotNullOfOrNull { find(it) }
        }
        val node = find(view.getJSONObject("root")) ?: return view
        val name = "fixture:" + node.getString("key")
        TranscriptNative.publish(name, node.toString())
        node.getJSONObject("element").getJSONObject("props").put("children", JSONArray()).put("source", name)
        return view
    }

    private val streamWords = ("The scheduler holds its queue lock while it waits for the next job, so every worker " +
        "blocks behind it. Releasing the lock before waiting lets the others make progress, and the stall under " +
        "load goes away. The retry loop also needs a bound, or a poisoned job spins forever.").split(' ')

    /**
     * Debug builds only (`rust_native_transcript_stream`): streams a long
     * assistant reply into the fixture's transcript, before its working
     * row, as a chat's streamed reply arrives. Each step adds three words;
     * every 60 words start a paragraph, and a code block grows by a line
     * every 12 steps from step 60. Returns false once the reply is done.
     */
    fun stream(view: JSONObject, step: Int, steps: Int): Boolean {
        fun find(node: JSONObject): JSONObject? {
            val element = node.getJSONObject("element")
            if (element.getString("kind") == "transcript") return element.getJSONObject("props")
            val children = element.optJSONObject("props")?.optJSONArray("children") ?: return null
            return children.objects().firstNotNullOfOrNull { find(it) }
        }
        val props = find(view.getJSONObject("root")) ?: return false
        val words = (step + 1) * 3
        val blocks = JSONArray()
        var written = 0
        while (written < words) {
            val take = minOf(60, words - written)
            val text = (written until written + take).joinToString(" ") { streamWords[it % streamWords.size] }
            blocks.put(JSONObject().put("kind", "paragraph").put("spans", JSONArray().put(JSONObject().put("text", text))))
            written += take
            if (written == 120 && step >= 60) {
                val lines = (0..minOf(8, (step - 60) / 12)).joinToString("") { "let job$it = queue.pop_front(); // line $it\n" }
                blocks.put(JSONObject().put("kind", "code").put("language", "rust").put("text", lines))
            }
        }
        val message = JSONObject().put("key", "stream").put("style", JSONObject())
            .put("element", JSONObject().put("kind", "message").put("props", JSONObject().put("role", "assistant").put("note", JSONObject.NULL)
                .put("children", JSONArray().put(JSONObject().put("key", "stream-md").put("style", JSONObject())
                    .put("element", JSONObject().put("kind", "markdown").put("props", JSONObject().put("blocks", blocks)))))))
        val children = props.getJSONArray("children")
        val rows = JSONArray()
        var placed = false
        for (i in 0 until children.length()) {
            val row = children.getJSONObject(i)
            if (row.getString("key") == "stream") continue
            if (row.getString("key") == "working") { rows.put(message); placed = true }
            rows.put(row)
        }
        if (!placed) rows.put(message)
        props.put("children", rows)
        view.put("revision", view.optLong("revision") + 1)
        return step + 1 < steps
    }

    private fun withRows(view: JSONObject, count: Int): JSONObject {
        if (count <= 0) return view
        fun find(node: JSONObject): JSONObject? {
            val element = node.getJSONObject("element")
            if (element.getString("kind") == "transcript") return element.getJSONObject("props")
            val children = element.optJSONObject("props")?.optJSONArray("children") ?: return null
            return children.objects().firstNotNullOfOrNull { find(it) }
        }
        val props = find(view.getJSONObject("root")) ?: return view
        fun span(text: String, flag: String? = null) = JSONObject().put("text", text).apply { flag?.let { put(it, true) } }
        fun paragraph(vararg spans: JSONObject) = JSONObject().put("kind", "paragraph").put("spans", JSONArray(spans.toList()))
        fun message(key: String, role: String, vararg blocks: JSONObject) = JSONObject().put("key", key).put("style", JSONObject())
            .put("element", JSONObject().put("kind", "message").put("props", JSONObject().put("role", role).put("note", JSONObject.NULL)
                .put("children", JSONArray().put(JSONObject().put("key", "$key-md").put("style", JSONObject())
                    .put("element", JSONObject().put("kind", "markdown").put("props", JSONObject().put("blocks", JSONArray(blocks.toList()))))))))
        val rows = JSONArray()
        for (index in 0 until count.coerceAtMost(20_000)) {
            val key = "s$index"
            rows.put(when (index % 4) {
                0 -> message(key, "user", paragraph(span("Question $index: why does the "), span("scheduler", "code"), span(" stall under load?")))
                1 -> message(key, "assistant", paragraph(span("The queue holds a lock while it "), span("waits", "italic"),
                    span(" for the next job, so every worker blocks behind it. "), span("Release the lock before waiting", "bold"), span(".")))
                2 -> message(key, "assistant", JSONObject().put("kind", "code").put("language", "rust")
                    .put("text", "let job = queue.lock().expect(\"the scheduler queue lock is poisoned\").pop_front().unwrap_or_default();\nready.wait();\n"))
                else -> message(key, "assistant", paragraph(span("Short reply $index.")))
            })
        }
        val own = props.getJSONArray("children")
        for (i in 0 until own.length()) rows.put(own.get(i))
        props.put("children", rows)
        return view
    }
}

/**
 * Debug builds only: a scripted fling. Two seconds after the first frame,
 * it scrolls the list up for four seconds and back down at a fixed speed on
 * the frame clock, then logs frame times under `TranscriptBench`. A hitch
 * is a frame later than 1.5 times the display's frame budget.
 */
internal class TranscriptBench(private val list: RecyclerView, private val begin: () -> Unit,
                                private val stats: () -> String) :
    android.view.Choreographer.FrameCallback {
    private val choreographer = android.view.Choreographer.getInstance()
    private val speed = 5_200f * list.resources.displayMetrics.density
    private val budget = 1_000_000_000L / (list.display?.refreshRate?.takeIf { it > 0 } ?: 60f).toLong()
    private var last = 0L
    private var direction = -1
    private var legs = 0
    private var legTime = 0L
    private val deltas = ArrayList<Long>()

    fun start() = list.postDelayed({ begin(); choreographer.postFrameCallback(this) }, 2_000)

    override fun doFrame(now: Long) {
        if (last != 0L) {
            val delta = now - last
            deltas.add(delta)
            legTime += delta
            val step = (speed * delta / 1e9f).toInt() * direction
            val moved = list.canScrollVertically(direction)
            list.scrollBy(0, step)
            if (!moved || legTime >= 4_000_000_000L) { direction = -direction; legs += 1; legTime = 0 }
        }
        last = now
        if (legs < 2) choreographer.postFrameCallback(this) else finish()
    }

    private fun finish() {
        if (deltas.isEmpty()) return
        val total = deltas.sum()
        val hitches = deltas.count { it > budget * 3 / 2 }
        android.util.Log.i("TranscriptBench", String.format(java.util.Locale.ROOT,
            "%d frames in %.2f s, mean %.2f ms, worst %.2f ms, %d hitches; %s", deltas.size, total / 1e9,
            total / 1e6 / deltas.size, deltas.max() / 1e6, hitches, stats()))
    }
}

/**
 * Debug builds only: frame times on the frame clock between start and
 * finish, logged under `tag`. A hitch is a frame later than 1.5 times the
 * display's frame budget.
 */
internal class TranscriptFrameStats(private val tag: String, private val view: View) : android.view.Choreographer.FrameCallback {
    private val choreographer = android.view.Choreographer.getInstance()
    private val budget = 1_000_000_000L / (view.display?.refreshRate?.takeIf { it > 0 } ?: 60f).toLong()
    private val deltas = ArrayList<Long>()
    private var last = 0L
    private var running = false

    fun start() { running = true; choreographer.postFrameCallback(this) }

    override fun doFrame(now: Long) {
        if (!running) return
        if (last != 0L) deltas.add(now - last)
        last = now
        choreographer.postFrameCallback(this)
    }

    fun finish() {
        running = false
        if (deltas.isEmpty()) return
        val total = deltas.sum()
        android.util.Log.i(tag, String.format(java.util.Locale.ROOT, "%d frames in %.2f s, mean %.2f ms, worst %.2f ms, %d hitches",
            deltas.size, total / 1e9, total / 1e6 / deltas.size, deltas.max() / 1e6, deltas.count { it > budget * 3 / 2 }))
    }
}
