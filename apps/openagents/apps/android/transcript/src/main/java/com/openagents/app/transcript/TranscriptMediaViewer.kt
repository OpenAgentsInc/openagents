// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.app.Dialog
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.view.Gravity
import android.view.GestureDetector
import android.view.ScaleGestureDetector
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageView
import uniffi.openagents_core.CoreSavedHistory
import uniffi.openagents_core.LabMediaFile
import uniffi.openagents_core.WidgetKind
import uniffi.openagents_core.maxAttachmentBytes

/** Reads transcript media through the Rust account and Lab readers. */
class TranscriptMediaLoader(private val history: CoreSavedHistory) {
    suspend fun image(target: TranscriptHitTarget.Widget): ByteArray? {
        val bytes = when (val kind = target.kind) {
            is WidgetKind.Image -> history.attachmentBytes(kind.reference)
            is WidgetKind.LabMedia -> {
                val artifact = history.labCard(kind.card)?.main ?: return null
                val media = history.labMedia(kind.card, artifact)
                (media as? LabMediaFile.Image)?.bytes ?: return null
            }
            else -> return null
        }
        return boundedTranscriptImage(bytes, maxAttachmentBytes())
    }
}

/** Loads through Rust and opens the native viewer when the target contains an image. */
suspend fun showTranscriptMedia(context: Context, history: CoreSavedHistory, target: TranscriptHitTarget.Widget): Boolean {
    val bytes = TranscriptMediaLoader(history).image(target) ?: return false
    return TranscriptMediaViewer(context, bytes).show()
}

/** Shows one Rust-loaded image. Zoom stays in the view; dismissal closes the dialog. */
class TranscriptMediaViewer(context: Context, private val imageBytes: ByteArray) {
    private val dialog = Dialog(context)
    private var bitmap: Bitmap? = null

    fun show(): Boolean {
        if (boundedTranscriptImage(imageBytes, maxAttachmentBytes()) == null) return false
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return false
        val options = BitmapFactory.Options().apply {
            inSampleSize = sampleSize(bounds.outWidth, bounds.outHeight)
        }
        bitmap = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, options) ?: return false

        val image = ZoomableImageView(dialog.context).apply {
            setImageBitmap(bitmap)
            contentDescription = "Image"
            setOnClickListener { dialog.dismiss() }
        }
        dialog.setContentView(FrameLayout(dialog.context).apply {
            addView(image, FrameLayout.LayoutParams(-1, -1, Gravity.CENTER))
            setOnClickListener { dialog.dismiss() }
        })
        dialog.window?.setBackgroundDrawableResource(android.R.color.black)
        dialog.window?.setLayout(-1, -1)
        dialog.setOnDismissListener {
            image.setImageDrawable(null)
            bitmap?.recycle()
            bitmap = null
        }
        dialog.show()
        dialog.window?.setLayout(-1, -1)
        return true
    }

    private fun sampleSize(width: Int, height: Int): Int {
        var sample = 1
        while (width.toLong() / sample * (height.toLong() / sample) > MAX_DECODE_PIXELS) sample *= 2
        return sample
    }

    private class ZoomableImageView(context: Context) : ImageView(context) {
        private val transform = Matrix()
        private var zoom = 1f
        private val detector = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                val next = (zoom * detector.scaleFactor).coerceIn(1f, MAX_ZOOM)
                val factor = next / zoom
                transform.postScale(factor, factor, detector.focusX, detector.focusY)
                zoom = next
                imageMatrix = transform
                return true
            }
        })
        private val taps = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapUp(event: android.view.MotionEvent): Boolean = performClick()
        })

        init {
            scaleType = ScaleType.MATRIX
            setBackgroundColor(android.graphics.Color.BLACK)
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            val image = drawable ?: return
            val scale = minOf(w.toFloat() / image.intrinsicWidth, h.toFloat() / image.intrinsicHeight)
            transform.reset()
            transform.setScale(scale, scale)
            transform.postTranslate((w - image.intrinsicWidth * scale) / 2f, (h - image.intrinsicHeight * scale) / 2f)
            zoom = 1f
            imageMatrix = transform
        }

        override fun onTouchEvent(event: android.view.MotionEvent): Boolean {
            detector.onTouchEvent(event)
            taps.onTouchEvent(event)
            return true
        }

        override fun performClick(): Boolean {
            super.performClick()
            return true
        }
    }

    private companion object {
        const val MAX_DECODE_PIXELS = 8_000_000L
        const val MAX_ZOOM = 5f
    }
}

internal fun boundedTranscriptImage(bytes: ByteArray, maxBytes: ULong): ByteArray? =
    bytes.takeIf { it.isNotEmpty() && it.size.toULong() <= maxBytes }
