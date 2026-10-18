// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import android.content.Context
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager

/** Reports Android audio, network, and proximity state for the active call. */
internal class AndroidVoiceAudioMonitor(context: Context) : AutoCloseable {
    private val appContext = context.applicationContext
    private val audio = appContext.getSystemService(AudioManager::class.java)
    private val connectivity = appContext.getSystemService(ConnectivityManager::class.java)
    private val sensors = appContext.getSystemService(SensorManager::class.java)
    private val mainHandler = Handler(Looper.getMainLooper())
    private val proximity = sensors.getDefaultSensor(Sensor.TYPE_PROXIMITY)
    @Volatile private var listener: ((VoiceMediaPlatformEvent) -> Unit)? = null
    @Volatile private var started = false
    private var near = false
    private var focusRequest: AudioFocusRequest? = null

    private val focusListener = AudioManager.OnAudioFocusChangeListener { change ->
        val state = when (change) {
            AudioManager.AUDIOFOCUS_GAIN -> VoiceAudioFocusState.GAINED
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> VoiceAudioFocusState.LOST_TRANSIENT_CAN_DUCK
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> VoiceAudioFocusState.LOST_TRANSIENT
            AudioManager.AUDIOFOCUS_LOSS -> VoiceAudioFocusState.LOST
            else -> return@OnAudioFocusChangeListener
        }
        listener?.invoke(VoiceMediaPlatformEvent.AudioFocusChanged(state))
    }

    private val deviceCallback = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = reportRoute()
        override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = reportRoute()
    }

    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            listener?.invoke(VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.AVAILABLE))
        }

        override fun onLost(network: Network) {
            val active = connectivity.activeNetwork
            val capabilities = active?.let(connectivity::getNetworkCapabilities)
            val available = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
            listener?.invoke(
                VoiceMediaPlatformEvent.NetworkChanged(
                    if (available) VoiceNetworkState.AVAILABLE else VoiceNetworkState.LOST,
                ),
            )
        }
    }

    private val proximityListener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) {
            val next = event.values.firstOrNull()?.let { it < event.sensor.maximumRange } ?: false
            if (near == next) return
            near = next
            applyRoute()
            listener?.invoke(VoiceMediaPlatformEvent.ProximityChanged(next))
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
    }

    fun start(eventListener: (VoiceMediaPlatformEvent) -> Unit) {
        if (started) return
        started = true
        listener = eventListener
        audio.mode = AudioManager.MODE_IN_COMMUNICATION
        val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE)
            .setOnAudioFocusChangeListener(focusListener, mainHandler)
            .setAcceptsDelayedFocusGain(false)
            .setWillPauseWhenDucked(false)
            .build()
        focusRequest = request
        val focus = audio.requestAudioFocus(request)
        listener?.invoke(
            VoiceMediaPlatformEvent.AudioFocusChanged(
                if (focus == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                    VoiceAudioFocusState.GAINED
                } else {
                    VoiceAudioFocusState.LOST
                },
            ),
        )
        audio.registerAudioDeviceCallback(deviceCallback, mainHandler)
        connectivity.registerDefaultNetworkCallback(networkCallback)
        proximity?.let { sensors.registerListener(proximityListener, it, SensorManager.SENSOR_DELAY_NORMAL) }
        applyRoute()
        reportRoute()
    }

    private fun applyRoute() {
        val externalOutput = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).any { device ->
            device.type in EXTERNAL_OUTPUT_TYPES
        }
        audio.isSpeakerphoneOn = !externalOutput && !near
    }

    private fun reportRoute() {
        val devices = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
        val route = when {
            devices.any { it.type in BLUETOOTH_OUTPUT_TYPES } -> VoiceAudioRoute.BLUETOOTH
            devices.any { it.type in WIRED_OUTPUT_TYPES } -> VoiceAudioRoute.WIRED_HEADSET
            audio.isSpeakerphoneOn -> VoiceAudioRoute.SPEAKER
            devices.any { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE } -> VoiceAudioRoute.EARPIECE
            else -> VoiceAudioRoute.UNKNOWN
        }
        listener?.invoke(VoiceMediaPlatformEvent.RouteChanged(route))
    }

    override fun close() {
        if (!started) return
        started = false
        listener = null
        runCatching { connectivity.unregisterNetworkCallback(networkCallback) }
        runCatching { audio.unregisterAudioDeviceCallback(deviceCallback) }
        sensors.unregisterListener(proximityListener)
        focusRequest?.let(audio::abandonAudioFocusRequest)
        focusRequest = null
        audio.isSpeakerphoneOn = false
        audio.mode = AudioManager.MODE_NORMAL
    }

    private companion object {
        val BLUETOOTH_OUTPUT_TYPES = setOf(
            AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
            AudioDeviceInfo.TYPE_BLE_HEADSET,
        )
        val WIRED_OUTPUT_TYPES = setOf(
            AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
            AudioDeviceInfo.TYPE_WIRED_HEADSET,
            AudioDeviceInfo.TYPE_USB_HEADSET,
        )
        val EXTERNAL_OUTPUT_TYPES = BLUETOOTH_OUTPUT_TYPES + WIRED_OUTPUT_TYPES
    }
}
