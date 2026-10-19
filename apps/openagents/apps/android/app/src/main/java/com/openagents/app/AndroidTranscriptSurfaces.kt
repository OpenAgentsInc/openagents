package com.openagents.app

import android.animation.ValueAnimator
import android.content.Context
import android.content.res.Configuration
import android.graphics.Rect
import android.view.View
import android.widget.FrameLayout
import com.openagents.app.platform.*
import com.openagents.app.transcript.*
import kotlinx.coroutines.CoroutineScope
import org.json.JSONObject
import uniffi.openagents_core.*

/** Native children occupy only current Rust widget rectangles; no transcript measurement occurs here. */
internal class AndroidTranscriptSurfaces(
    private val context: Context,
    private val canvas: TranscriptCanvas,
    private val transcript: TranscriptView,
    private val owner: CoreAndroidHost,
    private val generation: ULong,
    private val contentRevision: ULong,
    private val history: CoreSavedHistory?,
    private val scope: CoroutineScope,
    private val openPage: (String) -> Unit,
) : AutoCloseable {
    val view = FrameLayout(context).apply {
        clipChildren = true
        addView(canvas, FrameLayout.LayoutParams(-1, -1))
    }
    private data class Key(val row: ULong, val version: ULong, val widget: UInt)
    private class Mount(val view: View, val close: () -> Unit) {
        var bounds: Rect? = null
        var clip: Rect? = null
    }
    private val mounts = linkedMapOf<Key, Mount>()
    private val failed = mutableSetOf<Key>()
    private var queued: Runnable? = null
    private var closed = false
    private var lifetime = 0L
    private val density get() = context.resources.displayMetrics.density
    private val dark get() = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    private val reducedMotion get() = !ValueAnimator.areAnimatorsEnabled()

    fun refresh() {
        if (closed || queued != null) return
        val stamp = lifetime
        val task = Runnable {
            queued = null
            if (!closed && lifetime == stamp) update()
        }
        queued = task
        view.post(task)
    }
    private fun update() {
        val snapshot = canvas.interactionSnapshot() ?: return
        val frame = snapshot.frame
        val visible = linkedSetOf<Key>()
        for (row in frame.rowsIn(snapshot.top, snapshot.top + snapshot.height).take(128)) {
            val display = frame.display(row.index) ?: continue
            for (widget in display.widgets.take(256)) {
                if (visible.size >= 16) break
                if (widget.kind !is WidgetKind.Visual && widget.kind !is WidgetKind.LabMedia) continue
                if (!listOf(widget.x, widget.y, widget.w, widget.h).all(Float::isFinite)
                    || widget.w <= 0f || widget.h !in 1f..8192f || widget.w > 16384f) continue
                val scroller = widget.scroller?.let { display.scrollers.getOrNull(it.toInt()) ?: return@let null }
                if (widget.scroller != null && scroller == null) continue
                val offset = widget.scroller?.let { snapshot.horizontal(row.key, it) } ?: 0f
                val x = widget.x + (scroller?.x ?: 0f) - offset
                val y = row.y - snapshot.top + widget.y + (scroller?.y ?: 0f)
                val bounds = Rect((x * density).toInt() + canvas.paddingLeft, (y * density).toInt() + canvas.paddingTop,
                    ((x + widget.w) * density).toInt() + canvas.paddingLeft, ((y + widget.h) * density).toInt() + canvas.paddingTop)
                val viewport = Rect(canvas.paddingLeft, canvas.paddingTop, canvas.width - canvas.paddingRight, canvas.height - canvas.paddingBottom)
                if (scroller != null && !viewport.intersect(
                    (scroller.x * density).toInt() + canvas.paddingLeft,
                    ((row.y - snapshot.top + scroller.y) * density).toInt() + canvas.paddingTop,
                    ((scroller.x + scroller.w) * density).toInt() + canvas.paddingLeft,
                    ((row.y - snapshot.top + scroller.y + scroller.h) * density).toInt() + canvas.paddingTop)) continue
                val clip = Rect(bounds)
                if (!clip.intersect(viewport) || bounds.width() <= 0 || bounds.height() <= 0) continue
                val key = Key(row.key, row.version, widget.id)
                visible += key
                val mounted = mounts[key] ?: if (failed.contains(key)) null else try {
                    create(widget, row.key, frame.revision)?.also { mounted ->
                        try { view.addView(mounted.view); mounts[key] = mounted }
                        catch (error: Throwable) { mounted.close(); throw error }
                    }
                } catch (_: Exception) { failed += key; null }
                if (mounted != null) {
                    clip.offset(-bounds.left, -bounds.top)
                    if (mounted.bounds != bounds) {
                        mounted.view.layoutParams = FrameLayout.LayoutParams(bounds.width(), bounds.height()).apply {
                            leftMargin = bounds.left; topMargin = bounds.top
                        }
                        mounted.bounds = bounds
                    }
                    if (mounted.clip != clip) { mounted.view.clipBounds = clip; mounted.clip = clip }
                }
            }
        }
        mounts.keys.filter { it !in visible }.forEach { key -> runCatching { remove(key) } }
        failed.retainAll(visible)
    }
    private fun create(widget: Widget, row: ULong, revision: ULong): Mount? = when (widget.kind) {
        is WidgetKind.Visual -> {
            val session = owner.transcriptVisual(transcript, generation, contentRevision, revision, row, widget.id)
            if (session == null) null else {
                val owned = OwnedVisualLabSurface.create(context, session, dark, reducedMotion)
                try {
                    (owned.view as AndroidVisualView).layoutAt(widget.w, context.resources.configuration.fontScale)
                    Mount(owned.view, owned::close)
                } catch (error: Throwable) { owned.close(); throw error }
            }
        }
        is WidgetKind.LabMedia -> {
            // Resolve the current Rust widget again before requesting its registered surface.
            val effect = owner.transcriptWidget(transcript, generation, contentRevision, revision, row, widget.id) as? AndroidTranscriptEffect.Media
            val card = effect?.kind as? WidgetKind.LabMedia
            if (card == null || history == null) null else {
                val core = history.labInlineProjection(card.card)
                var bridge: RustNativeLabCardBridge? = null
                var transferred = false
                try {
                    val projection = core.snapshot()
                    val identity = JSONObject(projection.json.toString(Charsets.UTF_8))
                    val container = FrameLayout(context)
                    val renderer = AndroidLabMediaRenderer(context, core, identity.getString("instance"),
                        identity.getLong("revision").toULong(), transcript, scope, widget.h.toUInt(), dark, reducedMotion)
                    val mounted = RustNativeLabCardBridge(context, container, renderer) { action -> view.post { action() } }
                    bridge = mounted
                    transferred = true
                    mounted.show(core) { effect -> when (effect) {
                        is LabCardEffect.OpenPage -> openPage(effect.url)
                    } }
                    Mount(container, mounted::close)
                } catch (error: Throwable) {
                    try { bridge?.close() } finally { if (!transferred) try { core.dispose() } finally { core.destroy() } }
                    throw error
                }
            }
        }
        else -> null
    }
    private fun remove(key: Key) {
        val mounted = mounts.remove(key) ?: return
        try { mounted.close() } finally { view.removeView(mounted.view) }
    }
    fun retire() {
        lifetime += 1
        queued?.let { view.removeCallbacks(it) }; queued = null
        mounts.keys.toList().forEach { key -> runCatching { remove(key) } }
        failed.clear()
    }
    override fun close() {
        if (closed) return
        closed = true
        try { retire() } finally { view.removeView(canvas) }
    }
}
