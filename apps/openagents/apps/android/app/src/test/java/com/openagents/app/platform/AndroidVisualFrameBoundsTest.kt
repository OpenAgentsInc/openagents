// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.openagents_core.FrameImage
import uniffi.openagents_core.SurfaceFrame
import uniffi.openagents_core.VisualPaint

class AndroidVisualFrameBoundsTest {
    @Test
    fun frameImagesMustFitThePixelAndByteBounds() {
        assertTrue(isBoundedVisualFrame(frame(2u, 2u, ByteArray(16))))
        assertFalse(isBoundedVisualFrame(frame(2u, 2u, ByteArray(15))))
        assertFalse(isBoundedVisualFrame(frame(8193u, 1u, ByteArray(0))))
        assertFalse(isBoundedVisualFrame(frame(4096u, 4096u, ByteArray(0))))
        assertTrue(isBoundedVisualFrame(SurfaceFrame(1uL, null, VisualPaint("chart", emptyList()), emptyList())))
    }

    private fun frame(width: UInt, height: UInt, pixels: ByteArray) = SurfaceFrame(
        1uL,
        FrameImage(width, height, pixels),
        VisualPaint("", emptyList()),
        emptyList(),
    )
}
