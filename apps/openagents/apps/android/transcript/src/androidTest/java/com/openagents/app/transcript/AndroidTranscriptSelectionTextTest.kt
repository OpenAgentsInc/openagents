// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

/** Requires B1's Android test wiring; no device result is recorded here. */
@RunWith(AndroidJUnit4::class)
class AndroidTranscriptSelectionTextTest {
    @Test fun icuKeepsCombiningSurrogateAndJoinedEmojiClustersWhole() {
        val text = "Ae\u0301👩‍💻B"
        assertEquals(1, AndroidTranscriptBoundaries.boundary(text, 2, false))
        assertEquals(3, AndroidTranscriptBoundaries.boundary(text, 2, true))
        assertEquals(3, AndroidTranscriptBoundaries.boundary(text, 5, false))
        assertEquals(8, AndroidTranscriptBoundaries.boundary(text, 5, true))
    }
    @Test fun wordSelectionReturnsOnlyUtf16OffsetsInTheOriginalText() {
        val text = "hello e\u0301🙂 world"
        val range = AndroidTranscriptBoundaries.word(text, 1)!!
        assertEquals("hello", text.substring(range.first, range.last + 1))
        assertTrue(range.first >= 0 && range.last < text.length)
    }
}
