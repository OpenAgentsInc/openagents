package com.openagents.coder

import android.app.Service
import android.content.Intent
import android.os.IBinder

/**
 * The build without app/google-services.json: no Firebase, and push stays
 * off. Gradle uses this directory only when that file is absent.
 */
object PushPlatform {
    const val AVAILABLE = false
    fun start(deliver: (String) -> Unit, failed: (String) -> Unit) {}
    fun stop() {}
}

/** Declared in the manifest for Firebase builds; disabled in this one. */
class CoderMessagingService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null
}
