package com.openagents.app

import org.junit.Assert.*
import org.junit.Test

class GenerationOwnedResourceTest {
    @Test fun accountRetirementClosesPlatformResourcesWithoutAVisibleRoot() {
        val owner = GenerationOwnedResource<AutoCloseable>()
        var closes = 0
        owner.install(7u, AutoCloseable { closes++ })
        owner.retireFor(7u)
        assertEquals(0, closes)
        owner.retireFor(8u)
        owner.retireFor(8u)
        assertEquals(1, closes)
        assertNull(owner.value)
    }
    @Test fun synchronousTerminalCallbackCannotRetireAReplacement() {
        val owner = GenerationOwnedResource<AutoCloseable>()
        var replacementClosed = false
        val replacement = AutoCloseable { replacementClosed = true }
        lateinit var old: AutoCloseable
        old = AutoCloseable {
            assertNull(owner.value)
            owner.install(9u, replacement)
            assertFalse(owner.remove(old))
        }
        owner.install(8u, old)
        owner.retireFor(9u)
        assertSame(replacement, owner.value)
        assertFalse(replacementClosed)
    }
    @Test fun notificationDenialDoesNotRepeatOnRecreationOrChangeCallEligibility() {
        assertTrue(shouldRequestCallNotifications(35, granted = false, requested = false))
        assertFalse(shouldRequestCallNotifications(35, granted = false, requested = true))
        assertFalse(shouldRequestCallNotifications(35, granted = true, requested = false))
        assertFalse(shouldRequestCallNotifications(26, granted = false, requested = false))
    }
}
