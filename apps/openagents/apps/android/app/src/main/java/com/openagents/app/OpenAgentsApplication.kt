package com.openagents.app

import android.app.Application

class OpenAgentsApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        AndroidCoreBootstrap.initialize(this)
    }
}
