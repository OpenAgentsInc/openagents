package com.openagents.app

import org.junit.Assert.*
import org.junit.Test

class DebugFixtureRoutesTest {
    @Test fun acceptsOnlyTheExistingTranscriptLab() {
        assertTrue(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, "lab", "transcript")))
        assertTrue(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, "transcript", null)))
        assertTrue(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, "transcript", "transcript")))
    }
    @Test fun realAccountLaunchCannotSelectAFixture() {
        assertFalse(DebugFixtureRoutes.accepts(DebugLaunchExtras(false, "lab", "transcript")))
    }
    @Test fun unsupportedAccountAndLabRoutesAreNotFixtures() {
        for (route in listOf("wallet", "recovery", "history", "settings", "lab-note", "")) {
            assertFalse(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, route, "transcript")))
        }
        assertFalse(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, "lab", "wallet")))
        assertFalse(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, null, "transcript")))
        assertFalse(DebugFixtureRoutes.accepts(DebugLaunchExtras(true, "transcript", "unknown")))
    }
}
