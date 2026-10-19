// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import android.media.AudioDeviceInfo
import android.os.Build
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AndroidVoiceAudioMonitorTest {
    @Test
    fun api31UsesTheSelectedCommunicationDeviceRatherThanConnectedOutputs() {
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.S).apply {
            devices = listOf(device(AudioDeviceInfo.TYPE_BLUETOOTH_SCO), device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER))
            selected = device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        }
        val controller = VoiceCommunicationRouteController(routes) { false }

        assertEquals(VoiceAudioRoute.SPEAKER, controller.activeRoute())
        controller.select(nearEar = false)
        assertEquals(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, routes.selected?.type)
        assertEquals(VoiceAudioRoute.SPEAKER, controller.activeRoute())
    }

    @Test
    fun deniedBluetoothPermissionFallsBackToSpeakerAndNeverReportsConnectedBluetooth() {
        val bluetooth = device(AudioDeviceInfo.TYPE_BLUETOOTH_SCO)
        val speaker = device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.S).apply {
            devices = listOf(bluetooth, speaker)
        }
        val controller = VoiceCommunicationRouteController(routes) { false }

        controller.select(nearEar = false)

        assertEquals(speaker, routes.selected)
        assertEquals(VoiceAudioRoute.SPEAKER, controller.activeRoute())
        assertFalse(routes.selectedBluetooth)
    }

    @Test
    fun grantedBluetoothPermissionSelectsAndReportsTheActiveBluetoothDevice() {
        val bluetooth = device(AudioDeviceInfo.TYPE_BLUETOOTH_SCO)
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.S).apply {
            devices = listOf(bluetooth, device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER))
        }
        val controller = VoiceCommunicationRouteController(routes) { true }

        controller.select(nearEar = false)

        assertEquals(bluetooth, routes.selected)
        assertEquals(VoiceAudioRoute.BLUETOOTH, controller.activeRoute())
    }

    @Test
    fun legacyBluetoothIsReportedOnlyAfterScoBecomesActiveAndCloseReleasesIt() {
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.R).apply {
            hasBluetoothOutput = true
            hasEarpiece = true
        }
        val controller = VoiceCommunicationRouteController(routes) { true }

        controller.select(nearEar = false)
        assertTrue(routes.scoRequested)
        assertEquals(VoiceAudioRoute.EARPIECE, controller.activeRoute())

        routes.scoActive = true
        assertEquals(VoiceAudioRoute.BLUETOOTH, controller.activeRoute())
        controller.close()

        assertTrue(routes.scoStopped)
    }

    @Test
    fun failedLegacyScoFallsBackToSpeaker() {
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.R).apply {
            hasBluetoothOutput = true
            hasEarpiece = true
        }
        val controller = VoiceCommunicationRouteController(routes) { true }

        controller.select(nearEar = false)
        controller.legacyScoFailed()

        assertTrue(routes.speaker)
        assertEquals(VoiceAudioRoute.SPEAKER, controller.activeRoute())
    }

    @Test
    fun closeRemovesApi31RouteListenerAndClearsSelection() {
        val routes = FakeRoutePort(apiLevel = Build.VERSION_CODES.S).apply {
            devices = listOf(device(AudioDeviceInfo.TYPE_BUILTIN_EARPIECE))
        }
        val controller = VoiceCommunicationRouteController(routes) { false }
        var changes = 0
        controller.observeRouteChanges { changes += 1 }
        controller.select(nearEar = true)
        routes.emitSelected(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        assertEquals(1, changes)

        controller.close()

        assertEquals(0, routes.listenerCount)
        assertTrue(routes.cleared)
        assertEquals(null, routes.selected)
    }

    private class FakeRoutePort(override val apiLevel: Int) : VoiceAudioRoutePort {
        var devices = emptyList<VoiceCommunicationDevice>()
        var selected: VoiceCommunicationDevice? = null
        var hasBluetoothOutput = false
        var scoActive = false
        var scoRequested = false
        var scoStopped = false
        var speaker = false
        var hasWiredOutput = false
        var hasEarpiece = false
        var cleared = false
        private var listener: ((VoiceCommunicationDevice?) -> Unit)? = null
        val listenerCount get() = if (listener == null) 0 else 1
        val selectedBluetooth get() = selected?.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
            selected?.type == AudioDeviceInfo.TYPE_BLE_HEADSET

        override fun availableCommunicationDevices() = devices
        override fun selectedCommunicationDevice() = selected
        override fun selectCommunicationDevice(id: Int): Boolean {
            selected = devices.firstOrNull { it.id == id } ?: return false
            return true
        }
        override fun clearCommunicationDevice() {
            cleared = true
            selected = null
        }
        override fun bluetoothOutputConnected() = hasBluetoothOutput
        override fun bluetoothScoActive() = scoActive
        override fun startBluetoothSco() { scoRequested = true }
        override fun stopBluetoothSco() { scoStopped = true; scoActive = false }
        override fun speakerphoneOn() = speaker
        override fun setSpeakerphoneOn(enabled: Boolean) { speaker = enabled }
        override fun wiredOutputConnected() = hasWiredOutput
        override fun earpieceAvailable() = hasEarpiece
        override fun addCommunicationDeviceListener(listener: (VoiceCommunicationDevice?) -> Unit) {
            this.listener = listener
        }
        override fun removeCommunicationDeviceListener() { listener = null }
        fun emitSelected(type: Int) {
            selected = devices.firstOrNull { it.type == type }
            listener?.invoke(selected)
        }

        companion object {
            private var nextId = 0
            fun device(type: Int) = VoiceCommunicationDevice(++nextId, type)
        }
    }

    private companion object {
        fun device(type: Int) = FakeRoutePort.device(type)
    }
}
