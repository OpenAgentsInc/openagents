package com.openagents.app.accessibility

import android.test.InstrumentationTestCase
import uniffi.openagents_core.AccessibilityDirection
import uniffi.openagents_core.AccessibilityBounds
import uniffi.openagents_core.AccessibilityPage
import uniffi.openagents_core.AccessibilityRow

/** Synthetic Rust-port results. Real layout navigation is covered by the queued Rust fixtures. */
class TranscriptNavigationTest : InstrumentationTestCase() {
    private class Port : TranscriptNavigationPort {
        var closed = 0
        var revision = 9uL
        val calls = mutableListOf<AccessibilityDirection>()
        override fun page(revision: ULong, anchor: ULong, direction: AccessibilityDirection): AccessibilityPage {
            calls.add(direction)
            return AccessibilityPage(this.revision, listOf(AccessibilityRow(77uL, AccessibilityBounds(0f, 100f, 320f, 24f), "Synthetic answer", null)), true, true, "Earlier messages", "Later messages")
        }
        override fun close() { closed++ }
    }
    fun testTypedActionsForwardOnlyRustRowAndMeasuredCoordinate() {
        val port = Port(); val scrolls = mutableListOf<Triple<ULong, ULong, NativeBounds>>()
        val navigation = TranscriptNavigation(port) { revision, row, bounds -> scrolls.add(Triple(revision, row, bounds)) }
        assertEquals(2, navigation.actions(9uL, 12uL).size)
        assertTrue(navigation.perform(TranscriptNavigation.EARLIER, 9uL, 12uL))
        assertEquals(listOf(Triple(9uL, 77uL, NativeBounds(0f, 100f, 320f, 24f))), scrolls)
        assertFalse(navigation.perform(123, 9uL, 12uL))
        navigation.close(); assertEquals(1, port.closed)
    }
    fun testRetiredFrameAndMismatchedRevisionCannotScroll() {
        val port = Port(); var scrolls = 0
        val navigation = TranscriptNavigation(port) { _, _, _ -> scrolls++ }
        navigation.actions(9uL, 12uL); navigation.retire()
        assertFalse(navigation.perform(TranscriptNavigation.LATER, 9uL, 12uL))
        navigation.actions(9uL, 12uL); port.revision = 10uL
        assertFalse(navigation.perform(TranscriptNavigation.LATER, 9uL, 12uL))
        navigation.close(); assertFalse(navigation.perform(TranscriptNavigation.EARLIER, 9uL, 12uL))
        assertEquals(0, scrolls)
    }
}
