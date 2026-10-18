package com.openagents.app.accessibility

import android.animation.ValueAnimator
import android.content.Context
import org.json.JSONObject

/** Lifecycle effects are requests to existing Rust owners, not replacement account/session state. */
class AccessibilityLifecycle(
    private val semantics: NativeSemantics,
    private val effects: (JSONObject) -> Unit,
    private val environmentChanged: (fontScale: Float, density: Float, reducedMotion: Boolean) -> Unit,
) {
    fun configuration(context: Context) {
        val environment = semantics.environment(context.resources.configuration.fontScale, context.resources.displayMetrics.density, !ValueAnimator.areAnimatorsEnabled())
        environmentChanged(environment.getDouble("font_scale").toFloat(), environment.getDouble("density").toFloat(), environment.getBoolean("reduced_motion"))
    }
    /** Save only Rust's account/screen-bound focus key, never editor contents or grants. */
    fun saveFocus(): String? = semantics.bookmark()
    fun restoreFocus(bookmark: String): Boolean = semantics.restore(bookmark)
    fun rotation() = event("rotation")
    fun processRecreated() = event("process_recreated")
    fun background() = event("background")
    fun foreground() = event("foreground")
    fun offline() = event("offline")
    fun reconnected() = event("reconnected")
    fun permissionRevoked() = event("permission_revoked")
    private fun event(name: String) { effects(semantics.lifecycle(name)) }
}
