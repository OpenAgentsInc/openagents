package com.openagents.app

import android.content.Intent
import android.os.Bundle
import android.widget.FrameLayout
import com.openagents.app.transcript.*
import uniffi.openagents_core.*

/** Local transcript captures use the same Rust fixture and layout engine as iOS. */
internal object DebugFixtureRoutes {
    fun accepts(launch: DebugLaunchExtras): Boolean = launch.demo &&
        (launch.route == "transcript" && (launch.lab == null || launch.lab == "transcript") ||
            launch.route == "lab" && launch.lab == "transcript")

    fun mount(activity: MainActivity, application: OpenAgentsApplication,
              launch: DebugLaunchExtras, state: Bundle?): AndroidDebugFixture? =
        if (accepts(launch)) DebugTranscriptFixture(activity, application, state) else null
}

private const val ANCHOR = "debug-transcript-anchor"

/** No account, SDK client, media loader, or scripted network requests. */
internal class DebugTranscriptFixture(
    private val activity: MainActivity,
    private val application: OpenAgentsApplication,
    state: Bundle?,
) : AndroidDebugFixture {
    override val view = FrameLayout(activity).apply { setBackgroundColor(activity.getColor(R.color.oa_background)) }
    private var canvas: TranscriptCanvas? = null
    private var engine: TranscriptView? = null
    private var selection: TranscriptSelectionHost? = null
    private var relay: TranscriptFrameRelay? = null
    private var closed = false
    private var anchor = state?.getBundle(ANCHOR)?.let { saved ->
        val inside = saved.getFloat("inside", Float.NaN)
        if (inside.isFinite() && inside >= 0f) TranscriptViewportAnchor(
            saved.getLong("row").toULong(), inside, saved.getBoolean("following")) else null
    }

    override fun foreground() {
        if (closed || canvas != null) return
        val transcript = TranscriptCanvas(activity, application.transcriptFonts, TranscriptPalette { role ->
            activity.getColor(when (role) {
                ColorRole.TEXT_SECONDARY, ColorRole.TEXT_TERTIARY, ColorRole.TEXT_FAINT, ColorRole.TEXT_SOFT -> R.color.oa_secondary
                ColorRole.USER_BUBBLE, ColorRole.CODE_BACKGROUND, ColorRole.INLINE_CODE_BACKGROUND,
                ColorRole.TABLE_HEADER_BACKGROUND, ColorRole.CHIP_BACKGROUND, ColorRole.TOOL_BADGE,
                ColorRole.TOOL_WELL, ColorRole.AGENT_CARD, ColorRole.AGENT_TILE -> R.color.oa_surface
                ColorRole.RULE, ColorRole.CODE_BORDER, ColorRole.TABLE_BORDER, ColorRole.TOOL_RAIL,
                ColorRole.AGENT_CARD_BORDER -> R.color.oa_hairline
                else -> R.color.oa_text
            })
        })
        var bound: TranscriptView? = null
        val updates = TranscriptFrameRelay(transcript) { RustTranscriptFrame(requireNotNull(bound).frame()) }
        var host: TranscriptSelectionHost? = null
        try {
            val layout = TranscriptView(application.transcriptText, updates)
            bound = layout
            transcript.onViewportChanged = { width, scale -> layout.setViewport(width, scale) }
            host = TranscriptSelectionHost(activity, transcript, transcript::interactionSnapshot,
                AndroidTranscriptSelectionGeometry(application.transcriptFonts),
                object : TranscriptSelectionCallbacks {
                    override fun onSelection(token: TranscriptSelectionToken, range: TranscriptTextSelection?) = Unit
                    // Fixture links and widgets deliberately have no external side effects.
                    override fun onHit(token: TranscriptSelectionToken, target: TranscriptHitTarget) = Unit
                    override fun onSystemTextAction(intent: Intent) {
                        if (!closed && canvas === transcript) activity.startActivity(intent)
                    }
                }, 0x553B82F6, activity.getColor(R.color.oa_text))
            transcript.interactionObserver = host.observer("debug/transcript")
            // Rust owns markdown parsing, row order, shaping, wrapping, and selection geometry.
            layout.setDebugEntries(listOf(DebugEntry("layout-fixture", false, layoutFixtureMarkdown(), false)), false)
            anchor?.let(transcript::restoreViewportAnchor)
            canvas = transcript; engine = layout; selection = host; relay = updates
            // Canvas detachment must retire the listener and borrowed selection before its frame.
            transcript.onRelease = { retire(transcript) }
            view.addView(host, FrameLayout.LayoutParams(-1, -1))
            transcript.submitFrame(RustTranscriptFrame(layout.frame()))
        } catch (error: Exception) {
            transcript.onRelease = null
            canvas = null; engine = null; selection = null; relay = null
            updates.close()
            try { host?.close() } finally {
                try { bound?.close() } finally {
                    transcript.close()
                    view.removeAllViews()
                }
            }
            throw error
        }
    }

    private fun retire(expected: TranscriptCanvas) {
        if (canvas !== expected) return
        anchor = expected.saveViewportAnchor()
        val oldRelay = relay; relay = null
        val oldEngine = engine; engine = null
        val oldSelection = selection; selection = null
        canvas = null
        // Remove the Canvas release callback before SelectionHost closes its content.
        expected.onRelease = null
        oldRelay?.close()
        try { oldSelection?.close() } finally { oldEngine?.close() }
    }

    override fun background() {
        canvas?.let { current ->
            try { retire(current) } finally { current.close() }
        }
        view.removeAllViews()
    }
    override fun configurationChanged() { canvas?.refreshViewport() }
    override fun saveState(state: Bundle) {
        val current = canvas?.saveViewportAnchor() ?: anchor ?: return
        state.putBundle(ANCHOR, Bundle().apply {
            putLong("row", current.row.toLong()); putFloat("inside", current.inside)
            putBoolean("following", current.following)
        })
    }
    override fun close() { if (!closed) { closed = true; background() } }
}
