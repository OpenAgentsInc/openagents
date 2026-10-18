// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.openagents_core.FrameImage
import uniffi.openagents_core.SurfaceFrame
import uniffi.openagents_core.VisualPaint
import uniffi.openagents_core.VisualShape
import uniffi.openagents_core.Rgba

class AndroidVisualFrameBoundsTest {
    @Test
    fun frameImagesMustFitThePixelAndByteBounds() {
        assertTrue(isBoundedVisualFrame(frame(2u, 2u, ByteArray(16))))
        assertFalse(isBoundedVisualFrame(frame(2u, 2u, ByteArray(15))))
        assertFalse(isBoundedVisualFrame(frame(8193u, 1u, ByteArray(0))))
        assertFalse(isBoundedVisualFrame(frame(4096u, 4096u, ByteArray(0))))
        assertTrue(isBoundedVisualFrame(SurfaceFrame(1uL, null, VisualPaint("chart", emptyList(), emptyList()), emptyList())))
    }

    @Test
    fun rustVectorFallbackNeedsNoBitmapAndRejectsNonfiniteGeometry() {
        val shape = VisualShape.Rect(x = 0f, y = 0f, w = 12f, h = 8f, radius = 0f,
            color = Rgba(r = 0f, g = 0f, b = 0f, a = 1f))
        assertTrue(isBoundedVisualFrame(SurfaceFrame(1uL, null,
            VisualPaint("", emptyList(), listOf(shape)), emptyList())))
        val invalid = VisualShape.Rect(x = Float.NaN, y = 0f, w = 12f, h = 8f, radius = 0f,
            color = Rgba(r = 0f, g = 0f, b = 0f, a = 1f))
        assertFalse(isBoundedVisualFrame(SurfaceFrame(1uL, null,
            VisualPaint("", emptyList(), listOf(invalid)), emptyList())))
    }

    @Test
    fun sliderInputRejectsNonfiniteTracksAndBoundsFiniteDrag() {
        assertNull(boundedSliderFraction(Float.NaN, 0f, 10f))
        assertNull(boundedSliderFraction(5f, Float.POSITIVE_INFINITY, 10f))
        assertNull(boundedSliderFraction(5f, 0f, 0f))
        assertEquals(0f, boundedSliderFraction(-10f, 0f, 10f)!!, 0f)
        assertEquals(1f, boundedSliderFraction(20f, 0f, 10f)!!, 0f)
    }

    private fun frame(width: UInt, height: UInt, pixels: ByteArray) = SurfaceFrame(
        1uL,
        FrameImage(width, height, pixels),
        VisualPaint("", emptyList(), emptyList()),
        emptyList(),
    )
}
