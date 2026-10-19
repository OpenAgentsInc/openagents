// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidVoiceCallServiceHostTest {
    @Test
    fun failedBindStopsServiceAndOnlyTheCapturedCall() {
        val platform = FakeVoiceServicePlatform<String>().apply { bindResult = false }
        val attached = mutableListOf<Pair<String, String>>()
        val stoppedCalls = mutableListOf<String>()
        val host = host(platform, attached, stoppedCalls)

        assertThrows(IllegalStateException::class.java) { host.attach("first-call") }

        assertEquals(1, platform.startCount)
        assertEquals(1, platform.bindCount)
        assertEquals(0, platform.unbindCount)
        assertEquals(1, platform.stopCount)
        assertTrue(attached.isEmpty())
        assertEquals(listOf("first-call"), stoppedCalls)
    }

    @Test
    fun closeBeforeConnectStopsStartedServiceAndIgnoresLateConnection() {
        val platform = FakeVoiceServicePlatform<String>()
        val attached = mutableListOf<Pair<String, String>>()
        val stoppedCalls = mutableListOf<String>()
        val host = host(platform, attached, stoppedCalls)

        host.attach("first-call")
        val stale = platform.listeners.single()
        host.close()
        host.close()
        stale.onConnected("late-service")

        assertEquals(1, platform.startCount)
        assertEquals(1, platform.unbindCount)
        assertEquals(1, platform.stopCount)
        assertTrue(attached.isEmpty())
        assertTrue(stoppedCalls.isEmpty())
    }

    @Test
    fun staleConnectionCannotAttachToOrStopReplacementCall() {
        val platform = FakeVoiceServicePlatform<String>()
        val attached = mutableListOf<Pair<String, String>>()
        val stoppedCalls = mutableListOf<String>()
        val host = host(platform, attached, stoppedCalls)

        host.attach("first-call")
        val stale = platform.listeners.single()
        host.close()
        host.attach("replacement-call")
        val replacement = platform.listeners.last()

        stale.onConnected("stale-service")
        replacement.onConnected("replacement-service")

        assertEquals(listOf("replacement-service" to "replacement-call"), attached)
        assertTrue(stoppedCalls.isEmpty())
        assertEquals(2, platform.startCount)
        assertEquals(2, platform.bindCount)
        assertEquals(1, platform.stopCount)
        assertFalse(platform.listeners.first() === platform.listeners.last())
    }

    @Test
    fun lateConnectionAfterFailedBindCannotAffectReplacementCall() {
        val platform = FakeVoiceServicePlatform<String>().apply { bindResult = false }
        val attached = mutableListOf<Pair<String, String>>()
        val stoppedCalls = mutableListOf<String>()
        val host = host(platform, attached, stoppedCalls)

        assertThrows(IllegalStateException::class.java) { host.attach("failed-call") }
        val stale = platform.listeners.single()
        platform.bindResult = true
        host.attach("replacement-call")
        val replacement = platform.listeners.last()

        stale.onConnected("late-failed-service")
        replacement.onConnected("replacement-service")

        assertEquals(listOf("replacement-service" to "replacement-call"), attached)
        assertEquals(listOf("failed-call"), stoppedCalls)
        assertEquals(2, platform.startCount)
        assertEquals(2, platform.bindCount)
        assertEquals(1, platform.stopCount)
    }

    @Test
    fun disconnectedServiceUnbindsStopsAndTerminatesItsCapturedCall() {
        val platform = FakeVoiceServicePlatform<String>()
        val attached = mutableListOf<Pair<String, String>>()
        val detached = mutableListOf<String>()
        val stoppedCalls = mutableListOf<String>()
        val host = host(platform, attached, stoppedCalls, detached)

        host.attach("first-call")
        val connection = platform.listeners.single()
        connection.onConnected("voice-service")
        connection.onDisconnected()
        connection.onConnected("late-service")

        assertEquals(listOf("voice-service" to "first-call"), attached)
        assertEquals(listOf("voice-service"), detached)
        assertEquals(listOf("first-call"), stoppedCalls)
        assertEquals(1, platform.unbindCount)
        assertEquals(1, platform.stopCount)
    }

    private fun host(
        platform: FakeVoiceServicePlatform<String>,
        attached: MutableList<Pair<String, String>>,
        stoppedCalls: MutableList<String>,
        detached: MutableList<String> = mutableListOf(),
    ) = VoiceCallServiceHost(
        platform = platform,
        attach = { service, call -> attached += service to call },
        update = { _, _ -> },
        detach = detached::add,
        stopCall = stoppedCalls::add,
    )
}

private class FakeVoiceServicePlatform<S> : VoiceCallServicePlatform<S> {
    var bindResult = true
    var startCount = 0
    var bindCount = 0
    var unbindCount = 0
    var stopCount = 0
    val listeners = mutableListOf<VoiceCallServicePlatform.Listener<S>>()

    override fun start() {
        startCount++
    }

    override fun bind(listener: VoiceCallServicePlatform.Listener<S>): Boolean {
        bindCount++
        listeners += listener
        return bindResult
    }

    override fun unbind(listener: VoiceCallServicePlatform.Listener<S>) {
        unbindCount++
    }

    override fun stop() {
        stopCount++
    }
}
