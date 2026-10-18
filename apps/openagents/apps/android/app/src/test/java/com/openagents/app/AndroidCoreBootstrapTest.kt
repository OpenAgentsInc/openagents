package com.openagents.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidCoreBootstrapTest {
    @Test
    fun repeatedStartupCallbacksInstallOnce() {
        val state = BootstrapState()
        var calls = 0

        repeat(3) {
            state.initialize { calls += 1 }
        }

        assertEquals(1, calls)
        state.requireInitialized()
    }

    @Test
    fun processRecreationStartsUninitialized() {
        val firstProcess = BootstrapState()
        firstProcess.initialize {}
        firstProcess.requireInitialized()

        val recreatedProcess = BootstrapState()
        assertThrows(IllegalStateException::class.java) {
            recreatedProcess.requireInitialized()
        }
        recreatedProcess.initialize {}
        recreatedProcess.requireInitialized()
    }

    @Test
    fun missingAndInvalidInitializationAreRejectedAndCanRetry() {
        val state = BootstrapState()
        assertThrows(IllegalStateException::class.java) {
            state.requireInitialized()
        }

        var calls = 0
        assertThrows(IllegalArgumentException::class.java) {
            state.initialize {
                calls += 1
                throw IllegalArgumentException("invalid context")
            }
        }
        assertFalse(runCatching { state.requireInitialized() }.isSuccess)

        state.initialize { calls += 1 }
        state.requireInitialized()
        assertEquals(2, calls)
        assertTrue(runCatching { state.requireInitialized() }.isSuccess)
    }

    @Test
    fun missingApplicationContextIsRejected() {
        assertThrows(IllegalArgumentException::class.java) {
            requireApplicationContext(null)
        }
    }
}
