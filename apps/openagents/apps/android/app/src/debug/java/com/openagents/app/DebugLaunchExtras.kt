package com.openagents.app

import android.content.Intent

/** Launch controls used by local fixtures. This file is absent from release. */
internal data class DebugLaunchExtras(
    val demo: Boolean,
    val route: String?,
    val lab: String?,
) {
    companion object {
        fun read(intent: Intent) = DebugLaunchExtras(
            demo = intent.getBooleanExtra("openagents.demo", false),
            route = intent.getStringExtra("openagents.route"),
            lab = intent.getStringExtra("openagents.lab"),
        )
    }
}
