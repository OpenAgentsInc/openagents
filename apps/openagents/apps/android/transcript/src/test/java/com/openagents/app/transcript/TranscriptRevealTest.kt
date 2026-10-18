// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.assertEquals
import org.junit.Test

class TranscriptRevealTest {
    @Test
    fun revealUsesRustAgeAndLocalElapsedWithoutComparingClockOrigins() {
        assertEquals(0.5f, transcriptRevealAlpha(150u, 100u, 50, 200u), 0.001f)
        assertEquals(0f, transcriptRevealAlpha(100u, 200u, 50, 200u), 0.001f)
        assertEquals(1f, transcriptRevealAlpha(500u, 100u, 50, 200u), 0.001f)
        assertEquals(1f, transcriptRevealAlpha(null, 0u, 0, 200u), 0.001f)
    }
}
