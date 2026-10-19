// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.openagents_core.ColorRole
import uniffi.openagents_core.Decoration
import uniffi.openagents_core.RevealSlice
import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.TextRun

class TranscriptRevealTest {
    private val text = "ffi"
    private val run = TextRun(0u, 3u, 0f, 12f, 20f, 0u, ColorRole.TEXT, Decoration.NONE, null, 0u)

    private fun row(vararg slices: RevealSlice) = RowDisplay(
        1uL, 1uL, 100f, 20f, text, listOf(run), emptyList(), emptyList(), emptyList(), emptyList(), emptyList(),
        200u, text, revealSlices = slices.toList(),
    )

    @Test fun revealUsesRustAgeAndLocalElapsedWithoutComparingClockOrigins() {
        assertEquals(0.5f, transcriptRevealAlpha(150u, 100u, 50, 200u), 0.001f)
        assertEquals(0f, transcriptRevealAlpha(100u, 200u, 50, 200u), 0.001f)
        assertEquals(1f, transcriptRevealAlpha(500u, 100u, 50, 200u), 0.001f)
        assertEquals(1f, transcriptRevealAlpha(null, 0u, 0, 200u), 0.001f)
    }

    @Test fun mixedAgesUseMostVisibleAlphaForTheWholeShapedRun() {
        val display = row(RevealSlice(0u, 0u, 1u, 0u), RevealSlice(0u, 1u, 2u, 180u))
        assertEquals(1f, transcriptRunRevealAlpha(display, 0u, run, 0, 200u), 0f)
    }

    @Test fun staleRunIndexAndIncompleteOrOverlappingCoverageStayVisible() {
        assertEquals(1f, transcriptRunRevealAlpha(row(RevealSlice(1u, 0u, 3u, 0u)), 0u, run, 0, 200u), 0f)
        assertEquals(1f, transcriptRunRevealAlpha(row(RevealSlice(0u, 0u, 2u, 0u)), 0u, run, 0, 200u), 0f)
        assertEquals(1f, transcriptRunRevealAlpha(row(RevealSlice(0u, 0u, 2u, 0u), RevealSlice(0u, 1u, 2u, 0u)), 0u, run, 0, 200u), 0f)
    }

    @Test fun gapsAcrossSurrogatePairsStayVisible() {
        val emojiText = "A🙂B"
        val emojiRun = run.copy(len = 4u)
        val display = RowDisplay(
            2uL, 1uL, 100f, 20f, emojiText, listOf(emojiRun), emptyList(), emptyList(), emptyList(), emptyList(), emptyList(),
            200u, emojiText, revealSlices = listOf(RevealSlice(0u, 0u, 2u, 0u), RevealSlice(0u, 2u, 2u, 0u)),
        )
        assertEquals(1f, transcriptRunRevealAlpha(display, 0u, emojiRun, 0, 200u), 0f)
    }
}
