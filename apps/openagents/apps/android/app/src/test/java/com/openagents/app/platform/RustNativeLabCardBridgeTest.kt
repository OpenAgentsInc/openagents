// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import java.io.InputStreamReader
import java.nio.charset.StandardCharsets
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RustNativeLabCardBridgeTest {
    private class StubMount : RustNativeCardMount {
        var closes = 0
        lateinit var activation: (RustNativeCardActivation) -> Unit
        override fun close() { closes += 1 }
    }

    private class StubAdapter : RustNativeCardAdapter {
        val documents = mutableListOf<String>()
        val mounts = mutableListOf<StubMount>()
        override fun mount(viewJson: String, onActivation: (RustNativeCardActivation) -> Unit): RustNativeCardMount {
            val mount = StubMount().apply { activation = onActivation }
            documents += viewJson
            mounts += mount
            return mount
        }
    }

    @Test
    fun labFixturesPassThroughSharedViewsAndRetireOldCallbacks() {
        val adapter = StubAdapter()
        var mainQueue = mutableListOf<() -> Unit>()
        val bridge = RustNativeLabCardBridge(adapter) { action -> mainQueue += action }
        val events = mutableListOf<RustNativeCardActivation>()
        val fixtures = listOf("video", "compare", "model3d", "report").map(::fixture)

        fixtures.forEach { json -> bridge.show(json) { events += it } }

        assertEquals(fixtures, adapter.documents)
        assertEquals(3, adapter.mounts.take(3).count { it.closes == 1 })
        val oldActivation = RustNativeCardActivation("lab-video-1", 1uL, "video-open")
        adapter.mounts.first().activation(oldActivation)
        assertEquals(1, mainQueue.size)
        mainQueue.removeAt(0).invoke()
        assertTrue(events.isEmpty())

        val current = RustNativeCardActivation("lab-report-1", 1uL, "report-open")
        adapter.mounts.last().activation(current)
        mainQueue.removeAt(0).invoke()
        assertEquals(listOf(current), events)
        bridge.close()
        assertEquals(1, adapter.mounts.last().closes)
    }

    private fun fixture(name: String): String {
        val stream = checkNotNull(javaClass.getResourceAsStream("/fixtures/lab-$name-view.json"))
        return InputStreamReader(stream, StandardCharsets.UTF_8).use { it.readText() }
    }
}
