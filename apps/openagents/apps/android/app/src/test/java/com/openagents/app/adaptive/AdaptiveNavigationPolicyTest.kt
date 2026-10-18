package com.openagents.app.adaptive

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

class AdaptiveNavigationPolicyTest {
    @Test
    fun widthChangesKeepRustSessionDraftAndAnchorReferences() {
        val state = stubState()
        val policy = AdaptiveNavigationPolicy()

        val compact = policy.plan(
            AdaptiveWindowMetrics(widthDp = 412, heightDp = 860),
            state,
            AdaptivePane.SESSION,
        )
        val expanded = policy.plan(
            AdaptiveWindowMetrics(widthDp = 840, heightDp = 900),
            compact.retainedState,
            compact.activePane,
        )
        val compactAgain = policy.plan(
            AdaptiveWindowMetrics(widthDp = 480, heightDp = 860),
            expanded.retainedState,
            expanded.activePane,
        )

        assertEquals(AdaptivePaneMode.COMPACT, compact.mode)
        assertEquals(AdaptivePaneMode.EXPANDED, expanded.mode)
        assertEquals(AdaptivePaneMode.COMPACT, compactAgain.mode)
        assertSame(state, expanded.retainedState)
        assertSame(state.rustSession, compactAgain.retainedState.rustSession)
        assertSame(state.composerDraft, compactAgain.retainedState.composerDraft)
        assertSame(state.transcriptAnchor, compactAgain.retainedState.transcriptAnchor)
        assertEquals("chat-7", compactAgain.retainedState.selectedChatId)
        assertEquals("project-2", compactAgain.retainedState.selectedProjectId)
    }

    @Test
    fun measuredWidthControlsPaneModeAcrossRotationAndResize() {
        val policy = AdaptiveNavigationPolicy()
        val state = stubState()
        val widths = listOf(
            412 to AdaptivePaneMode.COMPACT,
            599 to AdaptivePaneMode.COMPACT,
            600 to AdaptivePaneMode.EXPANDED,
            840 to AdaptivePaneMode.EXPANDED,
        )

        widths.forEach { (width, expectedMode) ->
            val plan = policy.plan(
                AdaptiveWindowMetrics(widthDp = width, heightDp = 800),
                state,
                AdaptivePane.SESSION,
            )
            assertEquals(expectedMode, plan.mode)
            assertSame(state, plan.retainedState)
        }
    }

    @Test
    fun expandedPlanUsesBothPanesAndLeavesBackHandlingToHost() {
        val plan = AdaptiveNavigationPolicy().plan(
            AdaptiveWindowMetrics(widthDp = 840, heightDp = 900),
            stubState(),
            AdaptivePane.SESSION,
        )

        assertEquals(AdaptivePaneMode.EXPANDED, plan.mode)
        assertEquals(true, plan.showNavigation)
        assertEquals(true, plan.showSession)
        assertEquals(840, plan.navigationWidthDp + plan.sessionWidthDp)
        assertEquals(AdaptiveBackDisposition.DELEGATE_TO_HOST, plan.backDisposition)
    }

    @Test
    fun compactSessionBackReturnsToNavigationWithoutChangingOwnerState() {
        val state = stubState()
        val plan = AdaptiveNavigationPolicy().plan(
            AdaptiveWindowMetrics(widthDp = 412, heightDp = 860),
            state,
            AdaptivePane.SESSION,
        )

        assertEquals(AdaptiveBackDisposition.SHOW_NAVIGATION, plan.backDisposition)
        assertSame(state, plan.retainedState)
    }

    @Test
    fun windowInsetsAndLargeTextArePassedThroughWithoutChangingRetainedState() {
        val state = stubState()
        val plan = AdaptiveNavigationPolicy().plan(
            AdaptiveWindowMetrics(
                widthDp = 840,
                heightDp = 900,
                fontScale = 1.6f,
                imeBottomInsetDp = 360,
            ),
            state,
            AdaptivePane.SESSION,
        )

        assertEquals(1.6f, plan.fontScale, 0f)
        assertEquals(540, plan.contentHeightDp)
        assertEquals(840, plan.contentWidthDp)
        assertSame(state, plan.retainedState)
    }

    @Test
    fun narrowFoldableWindowUsesCompactNavigation() {
        val plan = AdaptiveNavigationPolicy().plan(
            AdaptiveWindowMetrics(
                widthDp = 580,
                heightDp = 780,
            ),
            stubState(),
            AdaptivePane.NAVIGATION,
        )

        assertEquals(580, plan.contentWidthDp)
        assertEquals(AdaptivePaneMode.COMPACT, plan.mode)
        assertEquals(true, plan.showNavigation)
        assertEquals(false, plan.showSession)
    }

    private fun stubState() = AdaptiveRetainedState(
        selectedChatId = "chat-7",
        selectedProjectId = "project-2",
        rustSession = StubRustSession("session-7"),
        composerDraft = StubRustDraft("draft-3"),
        transcriptAnchor = StubRustAnchor("row-19"),
    )

    private data class StubRustSession(val key: String)
    private data class StubRustDraft(val revision: String)
    private data class StubRustAnchor(val rowKey: String)
}
