// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.openagents_core.PushPrefs
import uniffi.openagents_core.PushWakeKind

class AndroidPushCoordinatorTest {
    @Test
    fun acceptsOnlyKnownSingleFieldWakeMessages() {
        assertEquals(AndroidPushWake.RECONNECT, AndroidPushWake.parse(mapOf("wake" to "reconnect")))
        assertEquals(AndroidPushWake.ATTENTION, AndroidPushWake.parse(mapOf("wake" to "attention")))
        assertNull(AndroidPushWake.parse(emptyMap()))
        assertNull(AndroidPushWake.parse(mapOf("wake" to "unknown")))
        assertNull(AndroidPushWake.parse(mapOf("wake" to "attention", "id" to "synthetic")))
    }

    @Test
    fun repeatedAndOutOfOrderWakeMessagesRemainHints() {
        val wakes = listOf(
            mapOf("wake" to "attention"),
            mapOf("wake" to "reconnect"),
            mapOf("wake" to "attention"),
        ).mapNotNull { AndroidPushWake.parse(it) }

        assertEquals(
            listOf(AndroidPushWake.ATTENTION, AndroidPushWake.RECONNECT, AndroidPushWake.ATTENTION),
            wakes,
        )
    }

    @Test
    fun wakeKindsMapToRustCommands() {
        assertEquals(PushWakeKind.RECONNECT, AndroidPushWake.RECONNECT.toRustKind())
        assertEquals(PushWakeKind.ATTENTION, AndroidPushWake.ATTENTION.toRustKind())
    }

    @Test
    fun tokenAndPreferencesReachRegistrationOnlyForTheCurrentClient() {
        val client = Any()
        val prefs = PushPrefs(done = true, input = false, failed = true)
        var readPrefs = false
        var forwarded: Triple<Any, String, PushPrefs>? = null

        assertTrue(
            registerPushTokenForCurrentClient(
                token = "synthetic-fcm-token",
                expected = client,
                current = client,
                canRegister = true,
                prefs = { readPrefs = true; prefs },
            ) { current, token, currentPrefs ->
                forwarded = Triple(current, token, currentPrefs)
            },
        )

        assertTrue(readPrefs)
        assertSame(client, forwarded?.first)
        assertEquals("synthetic-fcm-token", forwarded?.second)
        assertEquals(prefs, forwarded?.third)
    }

    @Test
    fun tokenRegistrationRejectsBlankDeniedAndReplacedClientsBeforeReadingPrefs() {
        val client = Any()
        val replacement = Any()
        val noPrefs: () -> PushPrefs = { error("Preferences must not be read") }
        val noRegistration: (Any, String, PushPrefs) -> Unit = { _, _, _ -> error("Must not register") }

        assertFalse(registerPushTokenForCurrentClient(" ", client, client, true, noPrefs, noRegistration))
        assertFalse(registerPushTokenForCurrentClient("token", client, client, false, noPrefs, noRegistration))
        assertFalse(registerPushTokenForCurrentClient("token", client, replacement, true, noPrefs, noRegistration))
    }

    @Test
    fun retiredClientCannotDeliverAWakeResultToItsReplacement() {
        val retired = Any()
        val replacement = Any()
        val delivered = mutableListOf<Any>()

        assertFalse(deliverToCurrentPushClient(retired, replacement) { delivered += it })
        assertTrue(delivered.isEmpty())
        assertTrue(deliverToCurrentPushClient(replacement, replacement) { delivered += it })
        assertEquals(listOf(replacement), delivered)
    }
}
