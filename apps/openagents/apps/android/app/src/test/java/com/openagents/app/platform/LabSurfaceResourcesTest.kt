// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.LabCardFiles
import uniffi.openagents_core.LabSurfaceRegistration

class LabSurfaceResourcesTest {
    private class Resource : AutoCloseable {
        var closes = 0
        var fail = false
        override fun close() { closes++; if (fail) throw IllegalStateException("synthetic close failure") }
    }
    private fun registered(id: String) = LabSurfaceRegistration(id,
        LabCardFiles("Test card", null, null, null, null, emptyList(), emptyList(), null, null, false))

    @Test fun registeredResourceMountsOnceAndMissingOrRetiredIdsNeverOpen() {
        val opened = mutableListOf<Resource>()
        val registry = LabSurfaceResources(listOf(registered("rust-resource"))) { Resource().also { opened += it } }
        val resource = registry.acquire("rust-resource")
        assertSame(resource, registry.acquire("rust-resource"))
        try { registry.acquire("foreign"); fail("missing resource") } catch (_: IllegalArgumentException) { }
        assertEquals(1, opened.size)
        registry.close(); registry.close()
        assertEquals(1, resource.closes)
        try { registry.acquire("rust-resource"); fail("retired resource") } catch (_: IllegalStateException) { }
        assertEquals(1, opened.size)
    }
    @Test fun replacementAndPartialFailureCloseEveryOwnedResource() {
        val old = Resource()
        val newer = Resource()
        val first = LabSurfaceResources(listOf(registered("same-id"))) { old }
        first.acquire("same-id")
        first.close()
        val replacement = LabSurfaceResources(listOf(registered("same-id"))) { newer }
        assertSame(newer, replacement.acquire("same-id"))
        assertEquals(0, newer.closes)
        val acquired = mutableListOf<Resource>()
        val partial = LabSurfaceResources(listOf(registered("a"), registered("b"))) {
            Resource().also { acquired += it }
        }
        partial.acquire("a").fail = true
        partial.acquire("b")
        try { partial.close(); fail("synthetic close failure") } catch (_: IllegalStateException) { }
        assertEquals(listOf(1, 1), acquired.map { it.closes })
        partial.close()
        replacement.close()
        assertEquals(1, old.closes)
        assertEquals(1, newer.closes)
    }
    @Test fun failedResourceCreationDoesNotRegisterAPhantomOwner() {
        var opens = 0
        val registry = LabSurfaceResources<Resource>(listOf(registered("a"))) {
            opens++; throw IllegalStateException("synthetic allocation failure")
        }
        try { registry.acquire("a"); fail("open failure") } catch (_: IllegalStateException) { }
        registry.close()
        assertEquals(1, opens)
    }
}
