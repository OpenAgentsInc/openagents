// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.VoiceCall
import uniffi.openagents_core.VoiceCallState

/** Typed handoff for the app shell to mount Rust Native call views and service controls. */
interface AndroidVoiceCallHostMount {
    fun mountCapsule(viewJson: String, activate: (instance: String, revision: Long, node: String) -> Unit)
    fun mountCallScreen(viewJson: String, activate: (instance: String, revision: Long, node: String) -> Unit)
    fun mountVoiceSettings(
        pickerJson: String,
        usageJson: String,
        activate: (instance: String, revision: Long, node: String) -> Unit,
    )
    fun dismissCallViews()
}

/** Keeps platform presentation separate from the Rust call owner and its authorization. */
class AndroidVoiceCallHost(
    private val context: Context,
    private val mount: AndroidVoiceCallHostMount,
) {
    private val orbSurface = AndroidVoiceOrbSurface(context)
    private var service: AndroidVoiceCallForegroundService? = null
    private var pendingCall: VoiceCall? = null
    private var latestState: VoiceCallState? = null
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            val connected = (binder as? AndroidVoiceCallForegroundService.LocalBinder)?.service() ?: return
            service = connected
            pendingCall?.let(connected::attach)
            pendingCall = null
            latestState?.let {
                orbSurface.update(it)
                connected.update(it)
            }
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            service = null
        }
    }

    /** Call after the app has obtained the Rust call and its first state snapshot. */
    fun attach(call: VoiceCall, state: VoiceCallState) {
        pendingCall = call
        AndroidVoiceCallForegroundService.start(context)
        context.bindService(
            Intent(context, AndroidVoiceCallForegroundService::class.java),
            connection,
            Context.BIND_AUTO_CREATE,
        )
        update(state)
    }

    /** Forward snapshots from VoiceSessionListener without keeping local call state. */
    fun update(state: VoiceCallState) {
        val snapshot = state.copy()
        latestState = snapshot
        orbSurface.update(snapshot)
        service?.update(snapshot)
    }

    /** Mounts views returned by voice_call_capsule_view/voice_call_screen_view. */
    fun mountViews(
        capsuleJson: String,
        callScreenJson: String,
        activate: (instance: String, revision: Long, node: String) -> Unit,
    ) {
        mount.mountCapsule(capsuleJson, activate)
        mount.mountCallScreen(callScreenJson, activate)
    }

    /** Mount CoreVoiceAgent.snapshot() with the Rust voice-usage projection. */
    fun mountVoiceSettings(
        pickerJson: String,
        usageJson: String,
        activate: (instance: String, revision: Long, node: String) -> Unit,
    ) {
        mount.mountVoiceSettings(pickerJson, usageJson, activate)
    }

    /** Register on the shell's SemanticViewAdapter when it mounts the call screen. */
    fun voiceOrbSurface(): SemanticViewAdapter.Extension = orbSurface

    fun close() {
        mount.dismissCallViews()
        service?.detach()
        service = null
        pendingCall = null
        latestState = null
        runCatching { context.unbindService(connection) }
    }
}
