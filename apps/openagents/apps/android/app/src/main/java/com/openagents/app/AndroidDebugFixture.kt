package com.openagents.app

import android.os.Bundle
import android.view.View

/** A local fixture mount; release launchers never construct one. */
internal interface AndroidDebugFixture : AutoCloseable {
    val view: View
    fun foreground()
    fun background()
    fun configurationChanged()
    fun saveState(state: Bundle)
}
