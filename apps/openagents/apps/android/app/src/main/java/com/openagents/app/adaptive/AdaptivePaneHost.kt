package com.openagents.app.adaptive

import android.content.Context
import android.os.Looper
import android.view.View

/** Keeps the mounted views while the shell reports window and keyboard changes. */
class AdaptivePaneHost<Session : Any, Draft : Any, Anchor : Any>(
    context: Context,
    navigation: View,
    session: View,
    private val metrics: () -> AdaptiveWindowMetrics,
    private val state: () -> AdaptiveRetainedState<Session, Draft, Anchor>,
    private val policy: AdaptiveNavigationPolicy = AdaptiveNavigationPolicy(),
) : AutoCloseable {
    val view = AdaptivePaneLayout(context)
    private var activePane = AdaptivePane.SESSION
    private var plan: AdaptiveLayoutPlan<Session, Draft, Anchor>? = null
    private var closed = false
    private val layoutListener = View.OnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> refresh() }

    init {
        ui()
        view.setPanes(navigation, session)
        view.addOnLayoutChangeListener(layoutListener)
    }

    /** Call after window insets or configuration change; never rebuild the panes. */
    fun refresh() {
        ui()
        if (closed) return
        val next = policy.plan(metrics(), state(), activePane)
        if (next == plan) return
        plan = next
        view.applyPlan(next)
    }

    fun showSession() { ui(); if (!closed) { activePane = AdaptivePane.SESSION; refresh() } }
    fun showNavigation() { ui(); if (!closed) { activePane = AdaptivePane.NAVIGATION; refresh() } }

    /** Returns false when the Activity should handle Back normally. */
    fun handleBack(): Boolean {
        ui()
        if (closed) return false
        refresh()
        if (plan?.backDisposition != AdaptiveBackDisposition.SHOW_NAVIGATION) return false
        showNavigation()
        return true
    }

    /** The account/screen owner, rather than this layout, closes Rust handles. */
    override fun close() {
        ui()
        if (closed) return
        closed = true
        view.removeOnLayoutChangeListener(layoutListener)
        view.removeAllViews()
        plan = null
    }

    private fun ui() { check(Looper.myLooper() == Looper.getMainLooper()) }
}
