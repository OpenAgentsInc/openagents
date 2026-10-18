// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TranscriptMediaViewerTest {
    @Test
    fun imagePayloadMustBeNonemptyAndWithinRustLimit() {
        val payload = byteArrayOf(1, 2, 3)
        assertArrayEquals(payload, boundedTranscriptImage(payload, 3uL))
        assertNull(boundedTranscriptImage(payload, 2uL))
        assertNull(boundedTranscriptImage(byteArrayOf(), 3uL))
    }
}
