package com.openagents.coder

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import org.json.JSONObject

/**
 * Push wakes are off unless this build carries Firebase configuration
 * (app/google-services.json) and names a relay, a push gateway, and an app
 * profile (see bins/coder-android/README.md). Rust owns registration, the
 * delivery grant, and the lease; the native side only obtains the FCM token.
 */
object PushSettings {
    val configured: Boolean
        get() = PushPlatform.AVAILABLE && BuildConfig.CODER_PUSH_RELAY_URL.isNotBlank() &&
            BuildConfig.CODER_PUSH_GATEWAY_URL.isNotBlank() && BuildConfig.CODER_PUSH_APP_PROFILE.isNotBlank()

    /** The `push` object of the Rust configuration. */
    fun rust(): JSONObject = json("relay_url" to BuildConfig.CODER_PUSH_RELAY_URL.trim(),
        "gateway_url" to BuildConfig.CODER_PUSH_GATEWAY_URL.trim(),
        "app_profile" to BuildConfig.CODER_PUSH_APP_PROFILE.trim())

    /** Ask for notification permission and pass each token to Rust. Does nothing unless configured. */
    fun start(activity: ComponentActivity, reader: ReaderBridge, permission: ActivityResultLauncher<String>) {
        if (!configured) return
        if (Build.VERSION.SDK_INT >= 33 &&
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            permission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        PushPlatform.start({ reader.pushToken(it) }, { reader.pushFailed(it) })
    }
}
