package com.openagents.app

import android.content.Intent

/** Release ignores local demo, route, and Lab launch options. */
internal data class DebugLaunchExtras(
    val demo: Boolean = false,
    val route: String? = null,
    val lab: String? = null,
) {
    companion object {
        fun read(@Suppress("UNUSED_PARAMETER") intent: Intent) = DebugLaunchExtras()
    }
}
