package com.openagents.app

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Outline
import android.view.View
import android.view.ViewOutlineProvider
import android.widget.FrameLayout
import android.widget.ImageView
import java.io.ByteArrayOutputStream

/** The chat's attached images: `image:` surfaces Rust names, drawn from the bytes Rust decoded. */
object ChatImages {
    private const val MAX_BYTES = 8 * 1024 * 1024
    private const val MAX_SIDE = 4096

    /** An attached image's card, 96 by 72 dp, rounded; its label is the spoken description. */
    fun card(context: Context, bridge: MobileBridge, resource: String): View {
        val image = ImageView(context).apply {
            scaleType = ImageView.ScaleType.CENTER_CROP
            background = context.rounded(Palette.RAISED, 12f, Palette.BORDER)
            clipToOutline = true
            outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) =
                    outline.setRoundRect(0, 0, view.width, view.height, context.dp(12).toFloat())
            }
            tag = resource
        }
        bridge.image(resource) { bitmap -> bitmap?.let { image.setImageBitmap(it) } }
        return FrameLayout(context).apply {
            addView(image, FrameLayout.LayoutParams(context.dp(96), context.dp(72)))
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        }
    }

    /**
     * A screenshot or image file a computer sent back (#11185), at the
     * screen's width and its own proportions; its label is the spoken
     * description.
     */
    fun capture(context: Context, bridge: MobileBridge, resource: String): View {
        val image = ImageView(context).apply {
            scaleType = ImageView.ScaleType.FIT_CENTER
            adjustViewBounds = true
            background = context.rounded(Palette.RAISED, 12f, Palette.BORDER)
            clipToOutline = true
            outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) =
                    outline.setRoundRect(0, 0, view.width, view.height, context.dp(12).toFloat())
            }
            minimumHeight = context.dp(120)
            tag = resource
        }
        bridge.image(resource, 2048) { bitmap -> bitmap?.let { image.setImageBitmap(it) } }
        return FrameLayout(context).apply {
            addView(image, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT))
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        }
    }

    /**
     * A picked photo as the PNG or JPEG Rust accepts: kept as is when it
     * already is one and fits, else re-encoded as a JPEG at most 4096 pixels
     * a side. Null when it isn't an image.
     */
    fun encoded(bytes: ByteArray): Pair<String, ByteArray>? {
        val png = bytes.size > 4 && bytes[0] == 0x89.toByte() && bytes[1] == 0x50.toByte() &&
            bytes[2] == 0x4E.toByte() && bytes[3] == 0x47.toByte()
        val jpeg = bytes.size > 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte() && bytes[2] == 0xFF.toByte()
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        val side = maxOf(bounds.outWidth, bounds.outHeight)
        if (side <= MAX_SIDE && bytes.size <= MAX_BYTES) {
            if (png) return "Photo.png" to bytes
            if (jpeg) return "Photo.jpg" to bytes
        }
        var sample = 1
        while (side / sample > MAX_SIDE * 2) sample *= 2
        val decoded = BitmapFactory.decodeByteArray(bytes, 0, bytes.size,
            BitmapFactory.Options().apply { inSampleSize = sample }) ?: return null
        val scale = minOf(1f, MAX_SIDE.toFloat() / maxOf(decoded.width, decoded.height))
        val bitmap = if (scale < 1f) Bitmap.createScaledBitmap(decoded,
            (decoded.width * scale).toInt().coerceAtLeast(1), (decoded.height * scale).toInt().coerceAtLeast(1), true)
        else decoded
        for (quality in listOf(85, 70, 50)) {
            val out = ByteArrayOutputStream()
            bitmap.compress(Bitmap.CompressFormat.JPEG, quality, out)
            if (out.size() <= MAX_BYTES) return "Photo.jpg" to out.toByteArray()
        }
        return null
    }
}
