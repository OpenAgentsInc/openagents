// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.CoreVoiceAgent
import uniffi.openagents_core.CoreVoiceUsage
import uniffi.openagents_core.VoiceCall
import uniffi.openagents_core.VoiceCallState
import java.util.IdentityHashMap

/** Typed handoff for the app shell to mount Rust Native call views and service controls. */
interface AndroidVoiceCallHostMount {
    fun mountCapsule(viewJson: String, call: VoiceCall)
    fun mountCallScreen(viewJson: String, call: VoiceCall)
    fun mountVoiceSettings(
        pickerJson: String,
        usageJson: String,
        picker: CoreVoiceAgent,
        usage: CoreVoiceUsage,
    )
    fun dismissCallViews()
}

/** Keeps platform presentation separate from the Rust call owner and its authorization. */
class AndroidVoiceCallHost(
    private val context: Context,
    private val mount: AndroidVoiceCallHostMount,
) {
    private val orbSurface = AndroidVoiceOrbSurface(context)
    private val serviceHost = VoiceCallServiceHost(
        platform = AndroidVoiceCallServicePlatform(context),
        attach = { service, call -> service.attach(call) },
        update = { service, state -> service.update(state) },
        detach = AndroidVoiceCallForegroundService::detach,
        stopCall = VoiceCall::stop,
        onState = orbSurface::update,
    )

    /** Start the microphone service while the permission-granting Activity is foreground. */
    fun attach(call: VoiceCall) {
        serviceHost.attach(call)
    }

    /** Compatibility entry for callers that already have the first Rust snapshot. */
    fun attach(call: VoiceCall, state: VoiceCallState) {
        attach(call)
        update(state)
    }

    /** Forward snapshots from VoiceSessionListener without keeping local call state. */
    fun update(state: VoiceCallState) {
        serviceHost.update(state)
    }

    /** Fetch and mount the active call's Rust-owned views. */
    fun mountViews(
        call: VoiceCall,
    ) {
        mount.mountCapsule(call.capsuleView(), call)
        mount.mountCallScreen(call.screenView(), call)
    }

    /** Mount CoreVoiceAgent.snapshot() with the Rust voice-usage projection. */
    fun mountVoiceSettings(
        picker: CoreVoiceAgent,
        usage: CoreVoiceUsage,
    ) {
        mount.mountVoiceSettings(picker.snapshot(), usage.viewJson(), picker, usage)
    }

    /** Register on the shell's SemanticViewAdapter when it mounts the call screen. */
    fun voiceOrbSurface(): SemanticViewAdapter.Extension = orbSurface

    fun close() {
        try {
            mount.dismissCallViews()
        } finally {
            serviceHost.close()
        }
    }
}

internal interface VoiceCallServicePlatform<S> {
    interface Listener<S> {
        fun onConnected(service: S)
        fun onDisconnected()
    }

    fun start()
    fun bind(listener: Listener<S>): Boolean
    fun unbind(listener: Listener<S>)
    fun stop()
}

/** Owns one service start and one bind attempt for the captured Rust call. */
internal class VoiceCallServiceHost<C, S>(
    private val platform: VoiceCallServicePlatform<S>,
    private val attach: (S, C) -> Unit,
    private val update: (S, VoiceCallState) -> Unit,
    private val detach: (S) -> Unit,
    private val stopCall: (C) -> Unit,
    private val onState: (VoiceCallState) -> Unit = {},
) {
    private inner class Attempt(val call: C) : VoiceCallServicePlatform.Listener<S> {
        var started = false
        var bindKnown = false
        var bound = false
        var closed = false
        var service: S? = null
        var latestState: VoiceCallState? = null

        override fun onConnected(service: S) {
            if (current !== this || closed) return
            this.service = service
            try {
                attach(service, call)
                latestState?.let {
                    onState(it)
                    update(service, it)
                }
            } catch (_: Exception) {
                fail(this)
            }
        }

        override fun onDisconnected() {
            if (current === this && !closed) fail(this)
        }
    }

    private var current: Attempt? = null

    fun attach(call: C) {
        check(current == null) { "A voice service binding is already active" }
        val attempt = Attempt(call)
        current = attempt
        try {
            attempt.started = true
            platform.start()
        } catch (error: Exception) {
            fail(attempt)
            throw error
        }
        try {
            val bound = platform.bind(attempt)
            attempt.bindKnown = true
            attempt.bound = bound
            if (!bound) {
                fail(attempt)
                throw IllegalStateException("Voice foreground service could not be bound")
            }
        } catch (error: Exception) {
            if (current === attempt) fail(attempt)
            throw error
        }
    }

    fun update(state: VoiceCallState) {
        val attempt = current ?: return
        if (attempt.closed) return
        val snapshot = state.copy()
        attempt.latestState = snapshot
        onState(snapshot)
        attempt.service?.let { update(it, snapshot) }
    }

    fun close() {
        current?.let { retire(it, stopCall = false) }
    }

    private fun fail(attempt: Attempt) {
        if (current !== attempt || attempt.closed) return
        retire(attempt, stopCall = true)
    }

    private fun retire(attempt: Attempt, stopCall: Boolean) {
        if (attempt.closed) return
        attempt.closed = true
        if (current === attempt) current = null
        if (attempt.bindKnown && attempt.bound) runCatching { platform.unbind(attempt) }
        attempt.service?.let { runCatching { detach(it) } }
        if (attempt.started) runCatching { platform.stop() }
        if (stopCall) runCatching { stopCall(attempt.call) }
        attempt.service = null
        attempt.latestState = null
    }
}

private class AndroidVoiceCallServicePlatform(
    private val context: Context,
) : VoiceCallServicePlatform<AndroidVoiceCallForegroundService> {
    private val connections = IdentityHashMap<VoiceCallServicePlatform.Listener<AndroidVoiceCallForegroundService>, ServiceConnection>()
    private fun intent() = Intent(context, AndroidVoiceCallForegroundService::class.java)

    override fun start() = AndroidVoiceCallForegroundService.start(context)

    override fun bind(listener: VoiceCallServicePlatform.Listener<AndroidVoiceCallForegroundService>): Boolean {
        val connection = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                val service = (binder as? AndroidVoiceCallForegroundService.LocalBinder)?.service() ?: return
                listener.onConnected(service)
            }

            override fun onServiceDisconnected(name: ComponentName?) = listener.onDisconnected()
        }
        connections[listener] = connection
        val bound = context.bindService(intent(), connection, Context.BIND_AUTO_CREATE)
        if (!bound) connections.remove(listener)
        return bound
    }

    override fun unbind(listener: VoiceCallServicePlatform.Listener<AndroidVoiceCallForegroundService>) {
        connections.remove(listener)?.let(context::unbindService)
    }

    override fun stop() {
        context.stopService(intent())
    }
}
