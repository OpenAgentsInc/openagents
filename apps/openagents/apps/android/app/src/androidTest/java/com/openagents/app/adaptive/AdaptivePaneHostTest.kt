package com.openagents.app.adaptive

import android.test.InstrumentationTestCase
import android.view.View

class AdaptivePaneHostTest : InstrumentationTestCase() {
    fun testResizeAndBackKeepMountedViewsAndOwnerReferences() {
        instrumentation.runOnMainSync {
            val context = instrumentation.targetContext
            val navigation = View(context)
            val session = View(context)
            val owner = Any()
            val draft = Any()
            val anchor = Any()
            val state = AdaptiveRetainedState("chat", "project", owner, draft, anchor)
            var metrics = AdaptiveWindowMetrics(412, 860)
            val host = AdaptivePaneHost(context, navigation, session, { metrics }, { state })

            host.refresh()
            assertEquals(View.GONE, navigation.visibility)
            assertEquals(View.VISIBLE, session.visibility)
            assertTrue(host.handleBack())
            assertEquals(View.VISIBLE, navigation.visibility)
            assertEquals(View.GONE, session.visibility)
            assertFalse(host.handleBack())

            metrics = AdaptiveWindowMetrics(840, 900, fontScale = 1.6f, imeBottomInsetDp = 360)
            host.refresh()
            assertEquals(View.VISIBLE, navigation.visibility)
            assertEquals(View.VISIBLE, session.visibility)
            assertFalse(host.handleBack())
            assertSame(host.view, navigation.parent)
            assertSame(host.view, session.parent)
            assertSame(navigation, host.view.getChildAt(0))
            assertSame(session, host.view.getChildAt(1))
            assertSame(owner, state.rustSession)
            assertSame(draft, state.composerDraft)
            assertSame(anchor, state.transcriptAnchor)

            metrics = AdaptiveWindowMetrics(412, 860)
            host.showSession()
            assertEquals(View.VISIBLE, session.visibility)
            host.close()
            assertNull(navigation.parent)
            assertNull(session.parent)
            assertFalse(host.handleBack())
        }
    }
}
