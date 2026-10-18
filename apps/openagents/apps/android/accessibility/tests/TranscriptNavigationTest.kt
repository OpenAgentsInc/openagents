package com.openagents.app.accessibility

import android.test.InstrumentationTestCase
import uniffi.openagents_core.AccessibilityDirection
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
            return AccessibilityPage(this.revision, listOf(AccessibilityRow(77uL, 100f, 24f, "Synthetic answer", null)), true, true, "Earlier messages", "Later messages")
        }
        override fun close() { closed++ }
    }
    fun testTypedActionsForwardOnlyRustRowAndMeasuredCoordinate() {
        val port = Port(); val scrolls = mutableListOf<Triple<ULong, ULong, Float>>()
        val navigation = TranscriptNavigation(port) { revision, row, y -> scrolls.add(Triple(revision, row, y)) }
        assertEquals(2, navigation.actions(9uL, 12uL).size)
        assertTrue(navigation.perform(TranscriptNavigation.EARLIER, 9uL, 12uL))
        assertEquals(listOf(Triple(9uL, 77uL, 100f)), scrolls)
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
