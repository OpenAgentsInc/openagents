package com.openagents.app.accessibility

import android.view.accessibility.AccessibilityNodeInfo
import uniffi.openagents_core.AccessibilityDirection
import uniffi.openagents_core.AccessibilityPage
import uniffi.openagents_core.CoreTranscriptAccessibility

/** Read-only Rust paging. The existing transcript host owns scrolling and virtual row nodes. */
interface TranscriptNavigationPort : AutoCloseable {
    fun page(revision: ULong, anchor: ULong, direction: AccessibilityDirection): AccessibilityPage
}
class CoreTranscriptNavigationPort(private val core: CoreTranscriptAccessibility) : TranscriptNavigationPort {
    override fun page(revision: ULong, anchor: ULong, direction: AccessibilityDirection) = core.page(revision, anchor, direction, 32u)
    override fun close() = core.destroy()
}

/** Adds navigation actions to the existing transcript, without installing a second node provider. */
class TranscriptNavigation(
    private val port: TranscriptNavigationPort,
    private val scrollTo: (revision: ULong, row: ULong, y: Float) -> Unit,
) : AutoCloseable {
    private var generation = 0L
    private var installed: Pair<ULong, ULong>? = null
    private var closed = false

    /** The host calls retire before replacing/closing the borrowed frame, including stream refresh. */
    fun retire() { generation++; installed = null }

    /** Mount alongside the existing virtual-row delegate. The host forwards these two actions. */
    fun actions(revision: ULong, anchor: ULong): List<AccessibilityNodeInfo.AccessibilityAction> {
        check(!closed)
        retire()
        val earlier = port.page(revision, anchor, AccessibilityDirection.EARLIER)
        val later = port.page(revision, anchor, AccessibilityDirection.LATER)
        require(earlier.revision == revision && later.revision == revision)
        installed = revision to anchor
        return buildList {
            if (earlier.rows.isNotEmpty()) add(AccessibilityNodeInfo.AccessibilityAction(EARLIER, earlier.earlierLabel))
            if (later.rows.isNotEmpty()) add(AccessibilityNodeInfo.AccessibilityAction(LATER, later.laterLabel))
        }
    }
    /** Call synchronously from the current frame's existing delegate. No queued viewport mutation. */
    fun perform(action: Int, revision: ULong, anchor: ULong): Boolean {
        if (closed || installed != (revision to anchor) || action !in setOf(EARLIER, LATER)) return false
        val stamp = generation
        val page = port.page(revision, anchor, if (action == EARLIER) AccessibilityDirection.EARLIER else AccessibilityDirection.LATER)
        val row = page.rows.firstOrNull() ?: return false
        if (closed || stamp != generation || page.revision != revision) return false
        scrollTo(revision, row.key, row.y)
        return true
    }
    override fun close() { if (!closed) { retire(); closed = true; port.close() } }
    companion object {
        // IDs in the application-defined action range, distinct from platform scroll actions.
        const val EARLIER = 0x01020001
        const val LATER = 0x01020002
    }
}
