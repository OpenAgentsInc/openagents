// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

/** Notification setup shared by the push flavor; permission prompts stay in the app UI. */
object AndroidPushNotifications {
    const val SESSION_CHANNEL = "openagents.session-updates"
    const val OWNER_NOTE_CHANNEL = "openagents.owner-notes"

    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannels(
            listOf(
                NotificationChannel(
                    SESSION_CHANNEL,
                    "Session updates",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
                NotificationChannel(
                    OWNER_NOTE_CHANNEL,
                    "Agent notices",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
            ),
        )
    }

    fun canPostNotifications(context: Context): Boolean =
        Build.VERSION.SDK_INT < 33 ||
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
}
