package com.openagents.app

import android.os.Bundle

/** Release launch options cannot mount local fixtures. */
internal object DebugFixtureRoutes {
    fun mount(
        @Suppress("UNUSED_PARAMETER") activity: MainActivity,
        @Suppress("UNUSED_PARAMETER") application: OpenAgentsApplication,
        @Suppress("UNUSED_PARAMETER") launch: DebugLaunchExtras,
        @Suppress("UNUSED_PARAMETER") state: Bundle?,
    ): AndroidDebugFixture? = null
}
