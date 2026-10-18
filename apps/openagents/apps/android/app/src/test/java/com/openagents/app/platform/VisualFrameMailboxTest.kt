// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.*
import org.junit.Test

class VisualFrameMailboxTest {
    @Test fun staleDrainAndLateWorkerCannotClearReplacementFrames() {
        val main = mutableListOf<() -> Unit>()
        val received = mutableListOf<String>()
        val queue = VisualFrameMailbox<String>(2, { main += it }, { _, frame -> received += frame })
        val old = queue.current()
        queue.offer(old, 1uL, "old")
        val current = queue.reset()
        queue.offer(current, 1uL, "new")
        queue.offer(old, 2uL, "late old")
        main.removeAt(0).invoke()
        assertTrue(received.isEmpty())
        main.removeAt(0).invoke()
        assertEquals(listOf("new"), received)
    }
    @Test fun queueCoalescesAndBoundsSurfacesWhileRotationRetiresTheBatch() {
        val main = mutableListOf<() -> Unit>()
        val received = mutableListOf<Int>()
        val queue = VisualFrameMailbox<Int>(2, { main += it }, { _, frame -> received += frame })
        val epoch = queue.current()
        queue.offer(epoch, 1uL, 1)
        queue.offer(epoch, 1uL, 2)
        queue.offer(epoch, 2uL, 3)
        queue.offer(epoch, 3uL, 4)
        assertEquals(1, main.size)
        main.removeAt(0).invoke()
        assertEquals(listOf(3, 4), received)
        queue.offer(epoch, 4uL, 5)
        queue.reset()
        main.removeAt(0).invoke()
        assertEquals(listOf(3, 4), received)
    }
    @Test fun replacementDuringDeliveryDropsRemainingOldBatch() {
        val main = mutableListOf<() -> Unit>()
        val received = mutableListOf<Int>()
        lateinit var queue: VisualFrameMailbox<Int>
        queue = VisualFrameMailbox(2, { main += it }, { _, frame -> received += frame; queue.reset() })
        queue.offer(queue.current(), 1uL, 1)
        queue.offer(queue.current(), 2uL, 2)
        main.single().invoke()
        assertEquals(listOf(1), received)
    }
    @Test fun pendingBytesAreBoundedAndAnOversizedFrameNeverSchedules() {
        val main = mutableListOf<() -> Unit>()
        val received = mutableListOf<String>()
        val queue = VisualFrameMailbox<String>(16, { main += it }, { _, value -> received += value },
            maxWeight = 4, weight = { it.length.toLong() })
        val epoch = queue.current()
        queue.offer(epoch, 1uL, "too large")
        assertTrue(main.isEmpty())
        queue.offer(epoch, 1uL, "abc")
        queue.offer(epoch, 2uL, "de")
        main.single().invoke()
        assertEquals(listOf("de"), received)
    }
    @Test fun bitmapFailureReleasesPartialAllocationAndUsesRustPaintFallback() {
        var closes = 0
        val result = visualRasterOrFallback(
            allocate = { "bitmap" },
            copy = { throw IllegalStateException("synthetic copy failure") },
            dispose = { closes++ },
        )
        assertNull(result)
        assertEquals(1, closes)
        assertNull(visualRasterOrFallback<String>(
            allocate = { throw OutOfMemoryError("synthetic allocation failure") },
            copy = {}, dispose = { closes++ },
        ))
        assertEquals(1, closes)
        assertEquals("bitmap", visualRasterOrFallback({ "bitmap" }, {}, { closes++ }))
        assertEquals(1, closes)
    }
}
