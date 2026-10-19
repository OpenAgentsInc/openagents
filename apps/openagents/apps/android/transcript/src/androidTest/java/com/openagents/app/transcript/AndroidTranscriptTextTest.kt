// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.text.Layout
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import android.test.InstrumentationTestCase
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import uniffi.openagents_core.*

/** Queued for B1's isolated API 26 and API 35 harness; no account or transport is used. */
class AndroidTranscriptTextTest : InstrumentationTestCase() {
    private fun fonts(): TranscriptFonts {
        return TranscriptFonts(instrumentation.targetContext.assets, TranscriptFonts.geistAssetPaths)
    }

    fun testFallbackMeasurementAndPainterUseTheSameRunMetrics() {
        val fonts = fonts()
        val measurer = AndroidTranscriptMeasurer(fonts)
        val samples = listOf("Latin ffi AV cafe", "漢字かな中文", "🙂 👨‍👩‍👧‍👦", "e\u0301 a\u0308", "العربية שלום", "Latin العربية 123 中文", "x != y -> z", "")
        for (face in listOf(FaceRole.SANS, FaceRole.SANS_BOLD, FaceRole.MONO)) for (sample in samples) {
            val ligatures = face != FaceRole.MONO
            val shaped = AndroidShapedText(sample, fonts.paint(face, 16.5f, ligatures))
            val advances = measurer.measureRun(face, 16.5f, ligatures, sample)
            assertEquals(sample.codePointCount(0, sample.length), advances.size)
            assertTrue(advances.all { it.isFinite() && it >= 0 })
            assertEquals(shaped.width, advances.sum(), .02f)
            assertEquals(shaped.width, measurer.measure(face, 16.5f, ligatures, sample), .001f)
        }
    }

    fun testRustWrappingAgreesWithAndroidForTheSameBundledBytesAndFallback() {
        val fonts = fonts()
        val text = TextSystem(fonts.faces, AndroidTranscriptMeasurer(fonts))
        try {
            for (sample in listOf("Latin words with ffi AV wrapping across a narrow viewport", "漢字かな中文漢字かな中文漢字かな中文", "🙂 🙂 👨‍👩‍👧‍👦 🙂 🙂", "e\u0301 a\u0308 e\u0301 a\u0308 e\u0301 a\u0308", "العربية שלום العربية שלום")) {
                for (width in listOf(140, 240)) for (size in listOf(16.5f, 21.45f)) {
                    val paint = TextPaint(fonts.paint(FaceRole.SANS, size, true))
                    // Test oracle only: production never uses Android to wrap rows.
                    val layout = StaticLayout.Builder.obtain(sample, 0, sample.length, paint, width)
                        .setIncludePad(false).setAlignment(Layout.Alignment.ALIGN_NORMAL)
                        .setTextDirection(TextDirectionHeuristics.FIRSTSTRONG_LTR)
                        .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
                        .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE).build()
                    val expected = (0 until layout.lineCount).map { layout.getLineStart(it).toUInt() }
                    assertEquals("$sample at $width/$size", expected, debugLineStarts(text, FaceRole.SANS, size, width.toFloat(), sample))
                }
            }
        } finally { text.close() }
    }

    fun testRustDisplayBaselinesReachCanvasWithoutRoundingOrRemeasuringRows() {
        val fonts = fonts()
        val text = TextSystem(fonts.faces, AndroidTranscriptMeasurer(fonts))
        val ready = CountDownLatch(1)
        val listener = object : LayoutListener { override fun frameReady(revision: ULong) { ready.countDown() } }
        val engine = TranscriptView(text, listener)
        try {
            engine.setViewport(280f, 1f)
            engine.setDebugEntries(listOf(DebugEntry("offline-metrics", false, "Latin ffi AV e\u0301 中文 🙂 العربية", false)), false)
            assertTrue(ready.await(5, TimeUnit.SECONDS))
            var frame = engine.frame()
            val until = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
            while (frame.rowCount() == 0u && System.nanoTime() < until) {
                frame.close(); Thread.sleep(10); frame = engine.frame()
            }
            try {
                assertTrue(frame.rowCount() > 0u)
                val row = frame.display(0u)!!
                val canvas = PositionCanvas(Bitmap.createBitmap(560, 800, Bitmap.Config.ARGB_8888))
                val painter = TranscriptPainter(fonts, TranscriptPalette { Color.BLACK })
                painter.setStyles(frame.styles())
                painter.draw(canvas, row, 2f) { 0f }
                for (run in row.runs) {
                    assertTrue(canvas.origins.any { (x, y) -> kotlin.math.abs(x - run.x) < .001f && kotlin.math.abs(y - run.baseline) < .001f })
                }
            } finally { frame.close() }
        } finally { engine.shutdown(); engine.close(); text.close() }
    }

    fun testMixedRevealAgesKeepOneFullAndroidShapingContextPerRun() {
        val fonts = fonts()
        val text = TextSystem(fonts.faces, AndroidTranscriptMeasurer(fonts))
        val ready = CountDownLatch(1)
        val engine = TranscriptView(text, object : LayoutListener {
            override fun frameReady(revision: ULong) { ready.countDown() }
        })
        try {
            val sample = "ffi AV العربية e\u0301"
            engine.setViewport(320f, 1f)
            engine.setDebugEntries(listOf(DebugEntry("reveal-shaping", false, sample, false)), false)
            assertTrue(ready.await(5, TimeUnit.SECONDS))
            val frame = engine.frame()
            try {
                val row = frame.display(0u)!!
                val slices = row.runs.flatMapIndexed { index, run ->
                    if (index != 0 || run.len < 2u) {
                        listOf(RevealSlice(index.toUInt(), run.start, run.len, 0u))
                    } else {
                        var split = run.start + run.len / 2u
                        val splitAt = split.toInt()
                        if (splitAt < row.text.length && row.text[splitAt].isLowSurrogate()) split++
                        if (split == run.start || split == run.start + run.len) {
                            listOf(RevealSlice(index.toUInt(), run.start, run.len, 0u))
                        } else listOf(
                            RevealSlice(index.toUInt(), run.start, split - run.start, 0u),
                            RevealSlice(index.toUInt(), split, run.start + run.len - split, 180u),
                        )
                    }
                }
                val metadataRow = row.copy(revealSlices = slices)
                val canvas = PositionCanvas(Bitmap.createBitmap(640, 160, Bitmap.Config.ARGB_8888))
                val painter = TranscriptPainter(fonts, TranscriptPalette { Color.BLACK })
                painter.setStyles(frame.styles())
                painter.draw(canvas, metadataRow, 1f, reducedMotion = false) { 0f }
                val expected = row.runs.sumOf { run ->
                    val start = run.start.toInt()
                    val style = frame.styles().first { it.id == run.style }
                    AndroidShapedText(
                        row.text.substring(start, start + run.len.toInt()),
                        fonts.paint(style.face, style.size, style.ligatures),
                    ).runs.size
                }
                assertEquals(expected, canvas.origins.size)
                assertEquals(row.runs.sumOf { it.len.toInt() }, canvas.contextUnits.sum())
            } finally { frame.close() }
        } finally { engine.shutdown(); engine.close(); text.close() }
    }

    private class PositionCanvas(bitmap: Bitmap) : Canvas(bitmap) {
        val origins = ArrayList<Pair<Float, Float>>()
        val contextUnits = ArrayList<Int>()
        override fun drawTextRun(text: CharArray, index: Int, count: Int, contextIndex: Int, contextCount: Int, x: Float, y: Float, isRtl: Boolean, paint: Paint) {
            origins.add(x to y)
            contextUnits.add(count)
            super.drawTextRun(text, index, count, contextIndex, contextCount, x, y, isRtl, paint)
        }
    }
}
