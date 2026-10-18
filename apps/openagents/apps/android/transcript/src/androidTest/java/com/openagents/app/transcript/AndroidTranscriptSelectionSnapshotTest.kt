// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import android.graphics.Color
import android.test.InstrumentationTestCase
import uniffi.openagents_core.*

/** Synthetic frames; requires C1 fonts and B1 Android test wiring. Not run yet. */
class AndroidTranscriptSelectionSnapshotTest : InstrumentationTestCase() {
    private class Frame(override val revision: ULong) : TranscriptFrame, AutoCloseable {
        var onClose: () -> Unit = {}
        var closed = false
        override val width = 320f
        override val totalHeight = 24f
        override fun styles() = emptyList<StyleDesc>()
        override fun placement(index: UInt) = if (index == 0u) RowPlacement(0u, 20uL, revision, 0f, 24f, RowKind.MARKDOWN) else null
        override fun indexOf(key: ULong) = if (key == 20uL) 0u else null
        override fun indexAt(y: Float) = if (y in 0f..24f) 0u else null
        override fun rowsIn(top: Float, bottom: Float) = listOf(placement(0u)!!)
        override fun display(index: UInt) = if (index == 0u) RowDisplay(20uL, revision, width, 24f, "hello", emptyList(),
            emptyList(), emptyList(), emptyList(), emptyList(), emptyList(), "hello") else null
        override fun close() { onClose(); closed = true }
    }
    fun testSnapshotAndObserverRetireStampsBeforeAnyBorrowedFrameCloses() {
        val instrumentation = instrumentation
        val context = instrumentation.targetContext
        instrumentation.runOnMainSync {
            val canvas = TranscriptCanvas(context, TranscriptFonts(context.assets, TranscriptFonts.geistAssetPaths), TranscriptPalette { Color.BLACK })
            val controller = TranscriptSelectionController(AndroidTranscriptBoundaries)
            val trace = mutableListOf<String>()
            canvas.interactionObserver = object : TranscriptInteractionObserver {
                override fun beforeFrameReplacement(next: TranscriptFrame) {
                    trace += "before-${next.revision}"
                    controller.submit(next, "synthetic-account/room")
                }
                override fun viewportChanged() { trace += "viewport" }
                override fun beforeFrameClose() { trace += "retire"; controller.close() }
            }
            val first = Frame(1uL)
            canvas.submitFrame(first)
            val stale = controller.token
            controller.selectRow(stale, 20uL)
            val second = Frame(2uL)
            first.onClose = {
                trace += "close-first"
                assertSame(second, controller.currentFrame)
                assertFalse(controller.accepts(stale))
                assertSame(second, canvas.interactionSnapshot()!!.frame)
            }
            canvas.submitFrame(second)
            assertTrue(first.closed)
            val snapshot = canvas.interactionSnapshot()!!
            assertSame(second, snapshot.frame)
            assertEquals(0f, snapshot.horizontal(20uL, 0u), 0f)
            assertFalse(controller.select(stale, TranscriptTextPoint(20uL, 0), TranscriptTextPoint(20uL, 1)))
            second.onClose = { trace += "close-second"; assertNull(controller.currentFrame) }
            canvas.close()
            assertNull(canvas.interactionSnapshot())
            assertEquals(listOf("before-1", "viewport", "before-2", "viewport", "close-first", "retire", "close-second"), trace)
        }
    }
}
