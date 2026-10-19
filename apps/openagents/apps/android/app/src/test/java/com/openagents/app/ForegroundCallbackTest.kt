package com.openagents.app

import org.junit.Assert.*
import org.junit.Test

class ForegroundCallbackTest {
    @Test fun visibleDeliveryAndTeardown() {
        val slot = ForegroundCallback<String>()
        val delivered = mutableListOf<String>()
        assertFalse(slot.deliver("before"))
        val mount = slot.install { delivered += it }
        assertTrue(slot.deliver("visible"))
        mount.close()
        assertFalse(slot.deliver("background"))
        assertEquals(listOf("visible"), delivered)
    }
    @Test fun retiredActivityCannotRemoveReplacement() {
        val slot = ForegroundCallback<String>()
        val old = mutableListOf<String>()
        val replacement = mutableListOf<String>()
        val retired = slot.install { old += it }
        slot.install { replacement += it }
        retired.close()
        assertTrue(slot.deliver("new"))
        assertTrue(old.isEmpty())
        assertEquals(listOf("new"), replacement)
        slot.clear()
        assertFalse(slot.deliver("signed out"))
    }
}
