// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.content.res.AssetManager
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.icu.text.BreakIterator
import android.os.Build
import java.text.Bidi
import java.util.Locale
import uniffi.openagents_core.FaceData
import uniffi.openagents_core.FaceRole
import uniffi.openagents_core.PlatformMeasurer

/** Read each APK asset once for Rust and load Android's face from that same asset. */
class TranscriptFonts(assets: AssetManager, assetPaths: Map<FaceRole, String>) {
    companion object {
        val geistAssetPaths = mapOf(
            FaceRole.SANS to "fonts/Geist.ttf",
            FaceRole.SANS_MEDIUM to "fonts/Geist-Medium.ttf",
            FaceRole.SANS_SEMIBOLD to "fonts/Geist-SemiBold.ttf",
            FaceRole.SANS_BOLD to "fonts/Geist-Bold.ttf",
            FaceRole.SANS_ITALIC to "fonts/Geist-Italic.ttf",
            FaceRole.SANS_MEDIUM_ITALIC to "fonts/Geist-MediumItalic.ttf",
            FaceRole.SANS_SEMIBOLD_ITALIC to "fonts/Geist-SemiBoldItalic.ttf",
            FaceRole.SANS_BOLD_ITALIC to "fonts/Geist-BoldItalic.ttf",
            FaceRole.MONO to "fonts/GeistMono.ttf",
            FaceRole.MONO_MEDIUM to "fonts/GeistMono-Medium.ttf",
            FaceRole.MONO_SEMIBOLD to "fonts/GeistMono-SemiBold.ttf",
            FaceRole.MONO_ITALIC to "fonts/GeistMono-Italic.ttf",
        )
    }
    val faces: List<FaceData>
    private val typefaces: Map<FaceRole, Typeface>
    init {
        require(assetPaths.keys == FaceRole.values().toSet()) { "All transcript font faces must be bundled" }
        faces = FaceRole.values().map { FaceData(it, assets.open(assetPaths.getValue(it)).use { stream -> stream.readBytes() }) }
        typefaces = assetPaths.mapValues { (_, path) -> Typeface.createFromAsset(assets, path) }
    }

    fun paint(face: FaceRole, size: Float, ligatures: Boolean) = Paint(
        Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG or Paint.LINEAR_TEXT_FLAG,
    ).apply {
        require(size.isFinite() && size > 0)
        typeface = typefaces.getValue(face)
        textSize = size
        textLocale = Locale.ROOT
        textAlign = Paint.Align.LEFT
        isFakeBoldText = false
        textSkewX = 0f
        letterSpacing = 0f
        hinting = Paint.HINTING_OFF
        fontFeatureSettings = if (ligatures) "'kern' 1, 'liga' 1, 'clig' 1" else "'kern' 1, 'liga' 0, 'clig' 0"
    }
}

/** UTF-16 Android advances folded into the Unicode scalar contract Rust expects. */
internal fun scalarAdvances(text: String, perUnit: FloatArray): List<Float> {
    require(perUnit.size == text.length)
    val result = ArrayList<Float>(text.codePointCount(0, text.length))
    var unit = 0
    while (unit < text.length) {
        val end = unit + Character.charCount(text.codePointAt(unit))
        var width = 0f
        for (i in unit until end) width += perUnit[i]
        result.add(width)
        unit = end
    }
    return result
}

internal data class DirectionalRun(val start: Int, val end: Int, val rtl: Boolean, val width: Float)

/** Shapes one Rust-selected text fragment, never wraps or moves a row baseline. */
internal class AndroidShapedText(val text: String, val paint: Paint) {
    val chars = text.toCharArray()
    val perUnit = FloatArray(chars.size)
    val runs: List<DirectionalRun>
    val width: Float

    init {
        val bidi = if (text.isEmpty()) null else Bidi(text, Bidi.DIRECTION_DEFAULT_LEFT_TO_RIGHT)
        val levels = ByteArray(bidi?.runCount ?: 0) { bidi!!.getRunLevel(it).toByte() }
        val ordered = Array<Any>(levels.size) { it }
        if (ordered.isNotEmpty()) Bidi.reorderVisually(levels, 0, ordered, 0, ordered.size)
        runs = ordered.map { value ->
            val i = value as Int
            val resolved = checkNotNull(bidi)
            val start = resolved.getRunStart(i)
            val end = resolved.getRunLimit(i)
            val rtl = resolved.getRunLevel(i) and 1 != 0
            val width = if (Build.VERSION.SDK_INT >= 29) {
                advances29(start, end, rtl)
            } else {
                // API 26–28 has run cursor advances, but not getTextRunAdvances.
                // Use the same directional context and ICU grapheme boundaries.
                val iterator = BreakIterator.getCharacterInstance(Locale.ROOT)
                iterator.setText(text.substring(start, end))
                var from = iterator.first()
                var to = iterator.next()
                while (to != BreakIterator.DONE) {
                    val a = paint.getRunAdvance(chars, start, end, start, end, rtl, start + from)
                    val b = paint.getRunAdvance(chars, start, end, start, end, rtl, start + to)
                    perUnit[start + from] = (b - a).coerceAtLeast(0f)
                    from = to
                    to = iterator.next()
                }
                paint.getRunAdvance(chars, start, end, start, end, rtl, end)
            }
            DirectionalRun(start, end, rtl, width)
        }
        width = runs.sumOf { it.width.toDouble() }.toFloat()
    }

    @android.annotation.TargetApi(29)
    private fun advances29(start: Int, end: Int, rtl: Boolean) = paint.getTextRunAdvances(
        chars, start, end - start, start, end - start, rtl, perUnit, start,
    )

    fun draw(canvas: Canvas, x: Float, baseline: Float) {
        var left = x
        for (run in runs) {
            canvas.drawTextRun(chars, run.start, run.end - run.start, run.start, run.end - run.start, left, baseline, run.rtl, paint)
            left += run.width
        }
    }
}

/** Each call owns its Paint and shaping state; Rust may call from a layout worker. */
class AndroidTranscriptMeasurer(private val fonts: TranscriptFonts) : PlatformMeasurer {
    override fun measure(face: FaceRole, size: Float, ligatures: Boolean, text: String): Float =
        AndroidShapedText(text, fonts.paint(face, size, ligatures)).width

    override fun measureRun(face: FaceRole, size: Float, ligatures: Boolean, text: String): List<Float> {
        val shaped = AndroidShapedText(text, fonts.paint(face, size, ligatures))
        return scalarAdvances(text, shaped.perUnit)
    }
}
