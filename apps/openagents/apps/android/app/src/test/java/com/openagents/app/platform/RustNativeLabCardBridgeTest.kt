// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.LabCardProjection
import uniffi.openagents_core.LabSurfaceRegistration

class RustNativeLabCardBridgeTest {
    private class StubMount : RustNativeLabMount {
        var closes = 0
        var fail = false
        var failClose = false
        val documents = mutableListOf<String>()
        lateinit var activation: (RustNativeCardActivation) -> Unit
        lateinit var fieldChanged: (String, Long, String, String) -> Unit
        lateinit var displayed: (String, Long) -> Unit
        override fun mount(json: ByteArray) {
            documents += String(json, Charsets.UTF_8)
            if (fail) throw IllegalStateException("synthetic mount failure")
        }
        override fun close() {
            closes++
            if (failClose) throw IllegalStateException("synthetic disposal failure")
        }
    }
    private class StubAdapter : RustNativeLabMountFactory {
        val mounts = mutableListOf<StubMount>()
        var failNext = false
        override fun create(
            surfaces: List<LabSurfaceRegistration>,
            onActivation: (RustNativeCardActivation) -> Unit,
            onFieldChanged: (String, Long, String, String) -> Unit,
            onDisplayed: (String, Long) -> Unit,
        ): RustNativeLabMount = StubMount().apply {
            fail = failNext
            failNext = false
            activation = onActivation
            fieldChanged = onFieldChanged
            displayed = onDisplayed
            mounts += this
        }
    }
    private fun fixture(name: String) = LabCardProjection(
        checkNotNull(javaClass.getResourceAsStream("/fixtures/lab-$name-view.json")).use { it.readBytes() },
        emptyList(), // Mount seam only; production resources come from CoreLabCard.snapshot().
    )
    @Test fun fixtureCardsRetireQueuedActivationFieldAndDisplayCallbacks() {
        val adapter = StubAdapter()
        val main = mutableListOf<() -> Unit>()
        val bridge = RustNativeLabCardBridge({ main += it }, adapter)
        val events = mutableListOf<RustNativeCardActivation>()
        val fields = mutableListOf<String>()
        val displayed = mutableListOf<String>()
        bridge.show(fixture("video"), onActivation = { events += it },
            onFieldChanged = { _, _, _, value -> fields += value }, onDisplayed = { instance, _ -> displayed += instance })
        val old = adapter.mounts.single()
        old.activation(RustNativeCardActivation("lab-video-1", 1L, "video-open"))
        old.fieldChanged("lab-video-1", 1L, "test-field", "stale")
        old.displayed("lab-video-1", 1L)
        bridge.show(fixture("report"), onActivation = { events += it })
        main.toList().forEach { it() }; main.clear()
        assertTrue(events.isEmpty()); assertTrue(fields.isEmpty()); assertTrue(displayed.isEmpty())
        assertEquals(1, old.closes)
        val current = RustNativeCardActivation("lab-report-1", 1L, "report-open")
        adapter.mounts.last().activation(current)
        main.removeAt(0).invoke()
        assertEquals(listOf(current), events)
        bridge.close(); bridge.close()
        assertEquals(1, adapter.mounts.last().closes)
    }
    @Test fun failedMountClosesOwnedResourcesAndCannotDeliverCallbacks() {
        val adapter = StubAdapter().apply { failNext = true }
        val main = mutableListOf<() -> Unit>()
        val bridge = RustNativeLabCardBridge({ main += it }, adapter)
        val events = mutableListOf<RustNativeCardActivation>()
        try { bridge.show(fixture("video"), onActivation = { events += it }); fail("mount must fail") }
        catch (_: IllegalStateException) { }
        val failed = adapter.mounts.single()
        assertEquals(1, failed.closes)
        failed.activation(RustNativeCardActivation("lab-video-1", 1, "video-open"))
        bridge.show(fixture("compare"), onActivation = { events += it })
        main.toList().forEach { it() }
        assertTrue(events.isEmpty())
        bridge.close()
        assertEquals(1, failed.closes)
        assertEquals(1, adapter.mounts.last().closes)
    }
    @Test fun replacementFailureStillRetiresPreviousMountAndTeardownIsTerminal() {
        val adapter = StubAdapter()
        val bridge = RustNativeLabCardBridge({ it() }, adapter)
        bridge.show(fixture("model3d"), onActivation = {})
        adapter.mounts.single().failClose = true
        try { bridge.show(fixture("report"), onActivation = {}); fail("dispose must fail") }
        catch (_: IllegalStateException) { }
        assertEquals(1, adapter.mounts.single().closes)
        bridge.close()
        try { bridge.show(fixture("report"), onActivation = {}); fail("closed bridge must reject reuse") }
        catch (_: IllegalStateException) { }
        assertEquals(1, adapter.mounts.size)
    }
    @Test fun oversizedProjectionNeverAcquiresAMount() {
        val adapter = StubAdapter()
        val bridge = RustNativeLabCardBridge({ it() }, adapter)
        try { bridge.show(LabCardProjection(ByteArray(512 * 1024 + 1), emptyList()), onActivation = {}); fail("bound") }
        catch (_: IllegalArgumentException) { }
        assertTrue(adapter.mounts.isEmpty())
    }
}
