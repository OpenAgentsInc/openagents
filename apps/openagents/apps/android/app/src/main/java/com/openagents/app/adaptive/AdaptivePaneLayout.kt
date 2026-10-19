package com.openagents.app.adaptive

import android.content.Context
import android.view.View
import android.widget.LinearLayout
import kotlin.math.roundToInt

/** Hosts navigation and session views without replacing either view on resize. */
class AdaptivePaneLayout(context: Context) : LinearLayout(context) {
    private var navigationView: View? = null
    private var sessionView: View? = null

    init {
        orientation = HORIZONTAL
    }

    fun setPanes(navigation: View, session: View) {
        require(navigation !== session)
        require(navigation.parent == null && session.parent == null) {
            "Adaptive panes must not already belong to a parent."
        }

        removeAllViews()
        navigationView = navigation
        sessionView = session
        addView(navigation, LayoutParams(0, LayoutParams.MATCH_PARENT))
        addView(session, LayoutParams(0, LayoutParams.MATCH_PARENT))
    }

    fun applyPlan(plan: AdaptiveLayoutPlan<*, *, *>) {
        val navigation = checkNotNull(navigationView) {
            "Set panes before applying a plan."
        }
        val session = checkNotNull(sessionView) {
            "Set panes before applying a plan."
        }

        navigation.visibility = if (plan.showNavigation) View.VISIBLE else View.GONE
        session.visibility = if (plan.showSession) View.VISIBLE else View.GONE

        if (plan.mode == AdaptivePaneMode.EXPANDED) {
            navigation.layoutParams = LayoutParams(
                dp(plan.navigationWidthDp),
                LayoutParams.MATCH_PARENT,
            )
            session.layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT, 1f)
        } else {
            val showNavigation = plan.activePane == AdaptivePane.NAVIGATION
            val activeView = if (showNavigation) navigation else session
            val inactiveView = if (showNavigation) session else navigation
            activeView.layoutParams = LayoutParams(
                LayoutParams.MATCH_PARENT,
                LayoutParams.MATCH_PARENT,
            )
            inactiveView.layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT)
        }
        requestLayout()
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).roundToInt()
}
