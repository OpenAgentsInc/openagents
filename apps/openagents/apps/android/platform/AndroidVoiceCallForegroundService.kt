// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import uniffi.openagents_core.VoiceCall
import uniffi.openagents_core.VoiceCallPhase
import uniffi.openagents_core.VoiceCallState

/** Keeps the existing Rust call handle alive and exposes native mute/end controls. */
class AndroidVoiceCallForegroundService : Service() {
    inner class LocalBinder : Binder() {
        fun service(): AndroidVoiceCallForegroundService = this@AndroidVoiceCallForegroundService
    }

    private val binder = LocalBinder()
    private val mainHandler = Handler(Looper.getMainLooper())
    private var call: VoiceCall? = null
    private var state: VoiceCallState? = null

    override fun onBind(intent: Intent?): IBinder = binder

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_MUTE -> call?.let { active ->
                val nextMuted = !(state?.muted ?: false)
                active.setMuted(nextMuted)
            }
            ACTION_END -> call?.stop()
        }
        if (intent?.action == ACTION_MUTE || intent?.action == ACTION_END) return START_NOT_STICKY
        ensureChannel()
        startForeground(NOTIFICATION_ID, notification(state))
        return START_NOT_STICKY
    }

    /** Called by the bound host on the main thread after Rust creates the call. */
    fun attach(call: VoiceCall) {
        this.call = call
        if (state == null) startForeground(NOTIFICATION_ID, notification(null))
    }

    /** The listener forwards Rust snapshots here; no notification state is persisted. */
    fun update(state: VoiceCallState) {
        val snapshot = state.copy()
        if (Looper.myLooper() != Looper.getMainLooper()) {
            mainHandler.post { updateOnMain(snapshot) }
        } else {
            updateOnMain(snapshot)
        }
    }

    private fun updateOnMain(state: VoiceCallState) {
        this.state = state
        if (state.phase == VoiceCallPhase.ENDING) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return
        }
        ensureChannel()
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.notify(NOTIFICATION_ID, notification(state))
    }

    fun detach() {
        call = null
        state = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun notification(state: VoiceCallState?): Notification {
        val muted = state?.muted ?: false
        val phase = when (state?.phase) {
            VoiceCallPhase.ACTIVE -> if (state.speaking) "Speaking" else if (muted) "Microphone off" else "Listening"
            VoiceCallPhase.ENDING -> "Ending call"
            else -> "Connecting"
        }
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle("Voice call")
            .setContentText(phase)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setOnlyAlertOnce(true)
            .addAction(0, if (muted) "Turn microphone on" else "Mute", action(ACTION_MUTE, 1))
            .addAction(0, "End call", action(ACTION_END, 2))
            .build()
    }

    private fun action(action: String, requestCode: Int): PendingIntent {
        val intent = Intent(this, AndroidVoiceCallForegroundService::class.java).setAction(action)
        return PendingIntent.getService(
            this,
            requestCode,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Voice calls", NotificationManager.IMPORTANCE_LOW),
        )
    }

    companion object {
        const val CHANNEL_ID = "openagents.voice-calls"
        const val NOTIFICATION_ID = 12097
        private const val ACTION_MUTE = "com.openagents.app.voice.MUTE"
        private const val ACTION_END = "com.openagents.app.voice.END"

        /** Start before binding so an ongoing call can outlive its activity. */
        fun start(context: Context) {
            val intent = Intent(context, AndroidVoiceCallForegroundService::class.java)
            ContextCompat.startForegroundService(context, intent)
        }
    }
}
