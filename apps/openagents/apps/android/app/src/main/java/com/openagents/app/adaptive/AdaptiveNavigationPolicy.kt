package com.openagents.app.adaptive

import kotlin.math.max
import kotlin.math.min

enum class AdaptivePane {
    NAVIGATION,
    SESSION,
}

enum class AdaptivePaneMode {
    COMPACT,
    EXPANDED,
}

enum class AdaptiveBackDisposition {
    SHOW_NAVIGATION,
    DELEGATE_TO_HOST,
}

data class AdaptiveWindowMetrics(
    val widthDp: Int,
    val heightDp: Int,
    val fontScale: Float = 1f,
    val imeBottomInsetDp: Int = 0,
)

/** Opaque references keep session state owned by Rust and its screen owner. */
data class AdaptiveRetainedState<Session : Any, Draft : Any, Anchor : Any>(
    val selectedChatId: String?,
    val selectedProjectId: String?,
    val rustSession: Session,
    val composerDraft: Draft,
    val transcriptAnchor: Anchor?,
)

data class AdaptiveLayoutPlan<Session : Any, Draft : Any, Anchor : Any>(
    val mode: AdaptivePaneMode,
    val activePane: AdaptivePane,
    val showNavigation: Boolean,
    val showSession: Boolean,
    val navigationWidthDp: Int,
    val sessionWidthDp: Int,
    val contentWidthDp: Int,
    val contentHeightDp: Int,
    val fontScale: Float,
    val backDisposition: AdaptiveBackDisposition,
    val retainedState: AdaptiveRetainedState<Session, Draft, Anchor>,
)

/** Calculates Android pane presentation from the measured window and owner state. */
class AdaptiveNavigationPolicy(
    private val expandedBreakpointDp: Int = 600,
    private val minimumNavigationWidthDp: Int = 280,
    private val minimumSessionWidthDp: Int = 320,
    private val preferredNavigationWidthDp: Int = 360,
) {
    init {
        require(expandedBreakpointDp > 0)
        require(minimumNavigationWidthDp > 0)
        require(minimumSessionWidthDp > 0)
        require(preferredNavigationWidthDp >= minimumNavigationWidthDp)
    }

    fun <Session : Any, Draft : Any, Anchor : Any> plan(
        metrics: AdaptiveWindowMetrics,
        retainedState: AdaptiveRetainedState<Session, Draft, Anchor>,
        activePane: AdaptivePane,
    ): AdaptiveLayoutPlan<Session, Draft, Anchor> {
        require(metrics.widthDp >= 0)
        require(metrics.heightDp >= 0)
        require(metrics.fontScale.isFinite() && metrics.fontScale > 0f)
        require(metrics.imeBottomInsetDp >= 0)

        val contentWidth = metrics.widthDp
        val contentHeight = max(0, metrics.heightDp - metrics.imeBottomInsetDp)
        val expanded = contentWidth >= max(
            expandedBreakpointDp,
            minimumNavigationWidthDp + minimumSessionWidthDp,
        )

        val mode = if (expanded) AdaptivePaneMode.EXPANDED else AdaptivePaneMode.COMPACT
        val navigationWidth = if (expanded) {
            min(preferredNavigationWidthDp, max(minimumNavigationWidthDp, contentWidth * 2 / 5))
        } else {
            contentWidth
        }
        val sessionWidth = if (expanded) contentWidth - navigationWidth else contentWidth

        return AdaptiveLayoutPlan(
            mode = mode,
            activePane = activePane,
            showNavigation = expanded || activePane == AdaptivePane.NAVIGATION,
            showSession = expanded || activePane == AdaptivePane.SESSION,
            navigationWidthDp = navigationWidth,
            sessionWidthDp = sessionWidth,
            contentWidthDp = contentWidth,
            contentHeightDp = contentHeight,
            fontScale = metrics.fontScale,
            backDisposition = if (!expanded && activePane == AdaptivePane.SESSION) {
                AdaptiveBackDisposition.SHOW_NAVIGATION
            } else {
                AdaptiveBackDisposition.DELEGATE_TO_HOST
            },
            retainedState = retainedState,
        )
    }
}
