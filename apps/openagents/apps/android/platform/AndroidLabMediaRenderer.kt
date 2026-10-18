// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.MediaController
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.VideoView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.openagents_core.CoreLabCardInterface
import uniffi.openagents_core.LabMediaFile
import uniffi.openagents_core.LabMediaView
import uniffi.openagents_core.LabSurfaceRegistration
import uniffi.openagents_core.TranscriptViewInterface

/** Native media widgets only. Rust supplies the files, authority, and card actions. */
class AndroidLabMediaRenderer(
    private val context: Context,
    private val core: CoreLabCardInterface,
    private val instance: String,
    private val revision: ULong,
    private val transcript: TranscriptViewInterface,
    private val uiScope: CoroutineScope,
    private val heightPoints: UInt,
    private val dark: Boolean,
    private val reduceMotion: Boolean,
) : LabSurfaceRenderer {
    init { require(heightPoints in 1u..8192u) }
    override fun open(surface: LabSurfaceRegistration): OwnedLabSurface {
        check(Looper.myLooper() == Looper.getMainLooper())
        val mounted = MediaMount(surface)
        try { mounted.start(); return mounted } catch (error: Throwable) {
            try { mounted.close() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        }
    }
    private inner class MediaMount(private val surface: LabSurfaceRegistration) : OwnedLabSurface {
        private var live = true
        private var job: Job? = null
        private val bitmaps = mutableListOf<Bitmap>()
        private var video: VideoView? = null
        private var model: OwnedVisualLabSurface? = null
        private val loading = ProgressBar(context)
        private val body = FrameLayout(context)
        private val root = object : FrameLayout(context) {
            override fun onDetachedFromWindow() {
                // Native playback pauses offscreen; Rust visual views detach their worker.
                video?.pause()
                super.onDetachedFromWindow()
            }
        }.apply {
            isSaveEnabled = false
            contentDescription = surface.files.title
            layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                (heightPoints.toFloat() * context.resources.displayMetrics.density).toInt())
            addView(body, FrameLayout.LayoutParams(-1, -1))
            addView(loading, FrameLayout.LayoutParams(-2, -2, android.view.Gravity.CENTER))
        }
        override val view: View get() = root
        private fun active(): Boolean = live && try { core.isCurrent() } catch (_: Exception) { false }
        private suspend fun media(artifact: String?): LabMediaFile {
            if (!active() || artifact == null) return LabMediaFile.Missing
            return core.media(instance, revision, surface.resource, artifact)
        }
        private suspend fun image(artifact: String?): ImageView? {
            val bytes = (media(artifact) as? LabMediaFile.Image)?.bytes ?: return null
            val decoded = java.util.concurrent.atomic.AtomicReference<Bitmap?>(null)
            val bitmap = try {
                withContext(Dispatchers.Default) {
                    if (bytes.size > 16 * 1024 * 1024) return@withContext
                    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
                    if (bounds.outWidth <= 0 || bounds.outHeight <= 0 || bounds.outWidth > 8192 || bounds.outHeight > 8192 ||
                        bounds.outWidth.toLong() * bounds.outHeight > 4 * 1024 * 1024) return@withContext
                    try { decoded.set(BitmapFactory.decodeByteArray(bytes, 0, bytes.size)) }
                    catch (_: OutOfMemoryError) { }
                }
                decoded.getAndSet(null)
            } finally {
                // Dispatcher cancellation must not discard an allocated bitmap.
                decoded.getAndSet(null)?.recycle()
            } ?: return null
            if (!active()) { bitmap.recycle(); return null }
            bitmaps += bitmap
            return ImageView(context).apply {
                isSaveEnabled = false
                scaleType = ImageView.ScaleType.FIT_CENTER
                setImageBitmap(bitmap)
                contentDescription = surface.files.title
            }
        }
        fun start() {
            job = uiScope.launch {
                try {
                    val files = surface.files
                    when (files.view) {
                        LabMediaView.COMPARE -> {
                            val first = image(files.sideA)
                            val second = image(files.sideB)
                            if (!active()) return@launch
                            val row = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL }
                            for ((index, picture) in listOf(first, second).withIndex()) {
                                val column = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
                                files.sideLabels.getOrNull(index)?.let { label -> column.addView(TextView(context).apply { text = label }) }
                                picture?.let { column.addView(it, LinearLayout.LayoutParams(-1, 0, 1f)) }
                                row.addView(column, LinearLayout.LayoutParams(0, -1, 1f))
                            }
                            body.addView(row, FrameLayout.LayoutParams(-1, -1))
                        }
                        LabMediaView.VIDEO -> {
                            val poster = image(files.main)
                            val source = media(files.plays) as? LabMediaFile.Video
                            if (!active()) return@launch
                            if (source != null) {
                                // The signed media URL comes only from the Rust SDK result.
                                val player = VideoView(context)
                                video = player
                                val controls = MediaController(context)
                                controls.setAnchorView(player)
                                player.setMediaController(controls)
                                player.setOnPreparedListener { if (live) { loading.visibility = View.GONE; controls.show() } }
                                player.setOnErrorListener { _, _, _ ->
                                    if (live) { player.visibility = View.GONE; poster?.visibility = View.VISIBLE }
                                    true
                                }
                                player.setVideoURI(Uri.parse(source.url))
                                body.addView(player, FrameLayout.LayoutParams(-1, -1))
                                poster?.setOnClickListener { if (live) { poster.visibility = View.GONE; player.start() } }
                            }
                            poster?.let { body.addView(it, FrameLayout.LayoutParams(-1, -1)) }
                        }
                        LabMediaView.MODEL3D -> {
                            val poster = image(files.main)
                            if (!active()) return@launch
                            poster?.let { body.addView(it, FrameLayout.LayoutParams(-1, -1)) }
                            val bytes = (media(files.model) as? LabMediaFile.Model)?.bytes
                            if (!active() || bytes == null) return@launch
                            val session = transcript.labModel(surface.resource, bytes, heightPoints,
                                context.resources.displayMetrics.density.coerceIn(0.5f, 2f)) ?: return@launch
                            // From this point the owned visual closes the returned native handle.
                            val owned = OwnedVisualLabSurface.create(context, session, dark, reduceMotion)
                            model = owned
                            if (!live) { owned.close(); model = null; return@launch }
                            body.removeAllViews()
                            body.addView(owned.view, FrameLayout.LayoutParams(-1, -1))
                            val visual = owned.view as AndroidVisualView
                            fun layout() {
                                val metrics = context.resources.displayMetrics
                                visual.layoutAt(body.width / metrics.density,
                                    metrics.scaledDensity / metrics.density)
                            }
                            body.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> if (live) layout() }
                            layout()
                        }
                        LabMediaView.IMAGE, LabMediaView.PLAYABLE -> {
                            val picture = image(files.main)
                            if (active() && picture != null) body.addView(picture, FrameLayout.LayoutParams(-1, -1))
                        }
                        null -> Unit
                    }
                } catch (cancelled: CancellationException) { throw cancelled }
                catch (_: Exception) {
                    // Shared Rust title/status/Open controls remain; never invent an error card.
                } finally {
                    if (live) loading.visibility = View.GONE
                }
            }
        }
        override fun close() {
            if (!live) return
            live = false
            job?.cancel()
            job = null
            try {
                video?.setOnPreparedListener(null)
                video?.setOnErrorListener(null)
                video?.stopPlayback()
            } finally {
                video = null
                try { model?.close() } finally {
                    model = null
                    body.removeAllViews()
                    root.removeAllViews()
                    bitmaps.forEach { it.recycle() }
                    bitmaps.clear()
                }
            }
        }
    }
}
