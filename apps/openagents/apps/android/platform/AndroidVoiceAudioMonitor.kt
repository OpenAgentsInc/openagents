// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import androidx.core.content.ContextCompat
import androidx.annotation.RequiresApi
import java.util.concurrent.Executor

internal data class VoiceCommunicationDevice(val id: Int, val type: Int)

/** AudioManager operations needed by route selection; the fake keeps route cases host-independent. */
internal interface VoiceAudioRoutePort {
    val apiLevel: Int
    fun availableCommunicationDevices(): List<VoiceCommunicationDevice>
    fun selectedCommunicationDevice(): VoiceCommunicationDevice?
    fun selectCommunicationDevice(id: Int): Boolean
    fun clearCommunicationDevice()
    fun bluetoothOutputConnected(): Boolean
    fun bluetoothScoActive(): Boolean
    fun startBluetoothSco()
    fun stopBluetoothSco()
    fun speakerphoneOn(): Boolean
    fun setSpeakerphoneOn(enabled: Boolean)
    fun wiredOutputConnected(): Boolean
    fun earpieceAvailable(): Boolean
    fun addCommunicationDeviceListener(listener: (VoiceCommunicationDevice?) -> Unit)
    fun removeCommunicationDeviceListener()
}

/** Selects routes and reports only the route Android says is active. */
internal class VoiceCommunicationRouteController(
    private val port: VoiceAudioRoutePort,
    private val bluetoothAllowed: () -> Boolean,
) {
    private var near = false
    private var requestedLegacySco = false

    fun observeRouteChanges(listener: (VoiceCommunicationDevice?) -> Unit) {
        if (port.apiLevel >= Build.VERSION_CODES.S) port.addCommunicationDeviceListener(listener)
    }

    fun legacyScoFailed() {
        if (port.apiLevel < Build.VERSION_CODES.S && requestedLegacySco) {
            requestedLegacySco = false
            port.setSpeakerphoneOn(!near && !port.wiredOutputConnected())
        }
    }

    fun select(nearEar: Boolean) {
        near = nearEar
        if (port.apiLevel >= Build.VERSION_CODES.S) {
            val available = port.availableCommunicationDevices()
            communicationDeviceCandidates(available, nearEar, bluetoothAllowed())
                .firstOrNull { port.selectCommunicationDevice(it.id) }
            return
        }

        val requestSco = bluetoothAllowed() && port.bluetoothOutputConnected()
        if (requestSco && !requestedLegacySco) {
            if (!port.bluetoothScoActive()) {
                requestedLegacySco = runCatching { port.startBluetoothSco(); true }.getOrDefault(false)
            }
        } else if (!requestSco && requestedLegacySco) {
            requestedLegacySco = false
            runCatching { port.stopBluetoothSco() }
        }
        if (!requestSco) port.setSpeakerphoneOn(!nearEar && !port.wiredOutputConnected())
    }

    fun activeRoute(): VoiceAudioRoute {
        if (port.apiLevel >= Build.VERSION_CODES.S) {
            return routeForDevice(port.selectedCommunicationDevice()?.type)
        }
        if (port.bluetoothScoActive()) return VoiceAudioRoute.BLUETOOTH
        if (port.speakerphoneOn()) return VoiceAudioRoute.SPEAKER
        if (port.wiredOutputConnected()) return VoiceAudioRoute.UNKNOWN
        return if (port.earpieceAvailable()) VoiceAudioRoute.EARPIECE else VoiceAudioRoute.UNKNOWN
    }

    fun close() {
        if (port.apiLevel >= Build.VERSION_CODES.S) {
            runCatching { port.removeCommunicationDeviceListener() }
            runCatching { port.clearCommunicationDevice() }
        } else {
            if (requestedLegacySco) runCatching { port.stopBluetoothSco() }
            requestedLegacySco = false
        }
    }

    companion object {
        internal fun chooseCommunicationDevice(
            devices: List<VoiceCommunicationDevice>,
            nearEar: Boolean,
            bluetoothAllowed: Boolean,
        ): VoiceCommunicationDevice? =
            communicationDeviceCandidates(devices, nearEar, bluetoothAllowed).firstOrNull()

        internal fun communicationDeviceCandidates(
            devices: List<VoiceCommunicationDevice>,
            nearEar: Boolean,
            bluetoothAllowed: Boolean,
        ): List<VoiceCommunicationDevice> {
            val preferredTypes = buildList {
                if (bluetoothAllowed) addAll(BLUETOOTH_OUTPUT_TYPES)
                addAll(WIRED_OUTPUT_TYPES)
                if (nearEar) add(AudioDeviceInfo.TYPE_BUILTIN_EARPIECE)
                add(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
                if (!nearEar) add(AudioDeviceInfo.TYPE_BUILTIN_EARPIECE)
            }
            return preferredTypes.mapNotNull { preferred ->
                devices.firstOrNull { it.type == preferred }
            }
        }

        internal fun routeForDevice(type: Int?): VoiceAudioRoute {
            if (type == null) return VoiceAudioRoute.UNKNOWN
            return when {
                type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> VoiceAudioRoute.EARPIECE
                type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> VoiceAudioRoute.SPEAKER
                type in WIRED_OUTPUT_TYPES -> VoiceAudioRoute.WIRED_HEADSET
                type in BLUETOOTH_OUTPUT_TYPES -> VoiceAudioRoute.BLUETOOTH
                else -> VoiceAudioRoute.UNKNOWN
            }
        }

        private val BLUETOOTH_OUTPUT_TYPES = listOf(
            AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
            AudioDeviceInfo.TYPE_BLE_HEADSET,
        )
        private val WIRED_OUTPUT_TYPES = listOf(
            AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
            AudioDeviceInfo.TYPE_WIRED_HEADSET,
            AudioDeviceInfo.TYPE_USB_HEADSET,
        )
    }
}

/** Reports the selected Android route, network, focus, and proximity for an active call. */
internal class AndroidVoiceAudioMonitor(
    context: Context,
    private val audio: AudioManager = context.applicationContext.getSystemService(AudioManager::class.java),
    private val connectivity: ConnectivityManager = context.applicationContext.getSystemService(ConnectivityManager::class.java),
    private val sensors: SensorManager = context.applicationContext.getSystemService(SensorManager::class.java),
) : AutoCloseable {
    private val appContext = context.applicationContext
    private val mainHandler = Handler(Looper.getMainLooper())
    private val proximity = sensors.getDefaultSensor(Sensor.TYPE_PROXIMITY)
    private var previousAudioMode = AudioManager.MODE_NORMAL
    private var previousSpeakerphone = false
    @Volatile private var listener: ((VoiceMediaPlatformEvent) -> Unit)? = null
    @Volatile private var started = false
    private var near = false
    private var focusRequest: AudioFocusRequest? = null
    private val routePort = AndroidAudioManagerRoutePort(audio, mainHandler)
    private val route = VoiceCommunicationRouteController(routePort) { hasBluetoothConnectPermission() }

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
        override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = selectAndReportRoute()
        override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = selectAndReportRoute()
    }

    private val legacyScoReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (!started || intent?.action != AudioManager.ACTION_SCO_AUDIO_STATE_UPDATED) return
            when (intent.getIntExtra(AudioManager.EXTRA_SCO_AUDIO_STATE, AudioManager.SCO_AUDIO_STATE_ERROR)) {
                AudioManager.SCO_AUDIO_STATE_CONNECTED -> reportRoute()
                AudioManager.SCO_AUDIO_STATE_DISCONNECTED, AudioManager.SCO_AUDIO_STATE_ERROR -> {
                    route.legacyScoFailed()
                    reportRoute()
                }
                else -> Unit
            }
        }
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
            selectAndReportRoute()
            listener?.invoke(VoiceMediaPlatformEvent.ProximityChanged(next))
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
    }

    fun start(eventListener: (VoiceMediaPlatformEvent) -> Unit) {
        if (started) return
        started = true
        listener = eventListener
        previousAudioMode = audio.mode
        previousSpeakerphone = audio.isSpeakerphoneOn
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
        ContextCompat.registerReceiver(
            appContext,
            legacyScoReceiver,
            IntentFilter(AudioManager.ACTION_SCO_AUDIO_STATE_UPDATED),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        route.observeRouteChanges { reportRoute() }
        proximity?.let { sensors.registerListener(proximityListener, it, SensorManager.SENSOR_DELAY_NORMAL) }
        selectAndReportRoute()
    }

    private fun hasBluetoothConnectPermission(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
            ContextCompat.checkSelfPermission(appContext, Manifest.permission.BLUETOOTH_CONNECT) ==
            PackageManager.PERMISSION_GRANTED

    private fun selectAndReportRoute() {
        if (!started) return
        route.select(near)
        reportRoute()
    }

    private fun reportRoute() {
        if (!started) return
        listener?.invoke(VoiceMediaPlatformEvent.RouteChanged(route.activeRoute()))
    }

    override fun close() {
        if (!started) return
        started = false
        listener = null
        runCatching { connectivity.unregisterNetworkCallback(networkCallback) }
        runCatching { audio.unregisterAudioDeviceCallback(deviceCallback) }
        runCatching { appContext.unregisterReceiver(legacyScoReceiver) }
        sensors.unregisterListener(proximityListener)
        focusRequest?.let(audio::abandonAudioFocusRequest)
        focusRequest = null
        route.close()
        audio.isSpeakerphoneOn = previousSpeakerphone
        audio.mode = previousAudioMode
    }
}

private class AndroidAudioManagerRoutePort(
    private val audio: AudioManager,
    private val handler: Handler,
) : VoiceAudioRoutePort {
    override val apiLevel: Int get() = Build.VERSION.SDK_INT
    private var communicationListener: Any? = null

    override fun availableCommunicationDevices(): List<VoiceCommunicationDevice> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) availableCommunicationDevicesApi31() else emptyList()

    @RequiresApi(Build.VERSION_CODES.S)
    private fun availableCommunicationDevicesApi31() =
        audio.availableCommunicationDevices.map { VoiceCommunicationDevice(it.id, it.type) }

    override fun selectedCommunicationDevice(): VoiceCommunicationDevice? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) selectedCommunicationDeviceApi31() else null

    @RequiresApi(Build.VERSION_CODES.S)
    private fun selectedCommunicationDeviceApi31() =
        audio.communicationDevice?.let { VoiceCommunicationDevice(it.id, it.type) }

    override fun selectCommunicationDevice(id: Int): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return false
        return selectCommunicationDeviceApi31(id)
    }

    @RequiresApi(Build.VERSION_CODES.S)
    private fun selectCommunicationDeviceApi31(id: Int): Boolean {
        val device = audio.availableCommunicationDevices.firstOrNull { it.id == id } ?: return false
        return runCatching { audio.setCommunicationDevice(device) }.getOrDefault(false)
    }

    override fun clearCommunicationDevice() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) clearCommunicationDeviceApi31()
    }

    @RequiresApi(Build.VERSION_CODES.S)
    private fun clearCommunicationDeviceApi31() = audio.clearCommunicationDevice()

    override fun bluetoothOutputConnected(): Boolean =
        audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).any {
            it.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO
        }

    override fun bluetoothScoActive(): Boolean = audio.isBluetoothScoOn
    override fun startBluetoothSco() = audio.startBluetoothSco()
    override fun stopBluetoothSco() = audio.stopBluetoothSco()
    override fun speakerphoneOn(): Boolean = audio.isSpeakerphoneOn
    override fun setSpeakerphoneOn(enabled: Boolean) { audio.isSpeakerphoneOn = enabled }
    override fun wiredOutputConnected(): Boolean = audio.isWiredHeadsetOn
    override fun earpieceAvailable(): Boolean = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).any {
        it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE
    }

    override fun addCommunicationDeviceListener(listener: (VoiceCommunicationDevice?) -> Unit) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return
        addCommunicationDeviceListenerApi31(listener)
    }

    @RequiresApi(Build.VERSION_CODES.S)
    private fun addCommunicationDeviceListenerApi31(listener: (VoiceCommunicationDevice?) -> Unit) {
        val registered = AudioManager.OnCommunicationDeviceChangedListener { device ->
            listener(device?.let { VoiceCommunicationDevice(it.id, it.type) })
        }
        communicationListener = registered
        audio.addOnCommunicationDeviceChangedListener(Executor { command -> handler.post(command) }, registered)
    }

    override fun removeCommunicationDeviceListener() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) removeCommunicationDeviceListenerApi31()
    }

    @RequiresApi(Build.VERSION_CODES.S)
    private fun removeCommunicationDeviceListenerApi31() {
        (communicationListener as? AudioManager.OnCommunicationDeviceChangedListener)
            ?.let(audio::removeOnCommunicationDeviceChangedListener)
        communicationListener = null
    }
}
