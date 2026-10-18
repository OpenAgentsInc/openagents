// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

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
}
