package com.openagents.app

import android.content.Intent
import android.os.Bundle
import android.test.InstrumentationTestCase
import android.widget.FrameLayout
import com.openagents.app.transcript.TranscriptCanvas
import com.openagents.app.transcript.TranscriptSelectionHost

/** Offline component fixture only; does not exercise account or Lab journeys. */
class AndroidDebugTranscriptFixtureTest : InstrumentationTestCase() {
    fun testFixtureRouteRequiresExplicitDemoOptIn() {
        val routeOnly = Intent()
            .putExtra("openagents.route", "lab")
            .putExtra("openagents.lab", "transcript")

        val launch = DebugLaunchExtras.read(routeOnly)
        assertFalse(launch.demo)
        assertFalse(DebugFixtureRoutes.accepts(launch))
    }

    fun testPauseResumeAndCloseRetireTheOldCanvasAndQueuedFrames() {
        val intent = Intent(instrumentation.targetContext, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            .putExtra("openagents.demo", true)
            .putExtra("openagents.route", "lab")
            .putExtra("openagents.lab", "transcript")
        val activity = instrumentation.startActivitySync(intent) as MainActivity
        try {
            instrumentation.waitForIdleSync()
            lateinit var container: FrameLayout
            lateinit var oldCanvas: TranscriptCanvas
            instrumentation.runOnMainSync {
                container = activity.findViewById<FrameLayout>(android.R.id.content).getChildAt(0) as FrameLayout
                val selection = container.getChildAt(0) as TranscriptSelectionHost
                oldCanvas = selection.getChildAt(0) as TranscriptCanvas
                instrumentation.callActivityOnPause(activity)
                assertEquals(0, container.childCount)
                assertNull(oldCanvas.interactionSnapshot())
            }
            // Worker notifications queued before pause must not repopulate the old view.
            instrumentation.waitForIdleSync()
            instrumentation.runOnMainSync {
                assertEquals(0, container.childCount)
                assertNull(oldCanvas.interactionSnapshot())
                instrumentation.callActivityOnResume(activity)
                val replacement = (container.getChildAt(0) as TranscriptSelectionHost).getChildAt(0)
                assertNotSame(oldCanvas, replacement)
                val saved = Bundle()
                instrumentation.callActivityOnSaveInstanceState(activity, saved)
                instrumentation.callActivityOnPause(activity)
                assertEquals(0, container.childCount)
            }
        } finally {
            instrumentation.runOnMainSync { activity.finish() }
            instrumentation.waitForIdleSync()
        }
    }
}
