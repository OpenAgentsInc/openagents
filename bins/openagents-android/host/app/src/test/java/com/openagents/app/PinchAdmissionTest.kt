package com.openagents.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PinchAdmissionTest {
    @Test fun aHeldOrMovedContactNeverBecomesAPinch() {
        val admission = PinchAdmission()
        admission.down(1, 100f, 400f, 1000)
        admission.move(1, 100f, 350f)
        admission.down(2, 300f, 400f, 1050)
        assertFalse(admission.reserved)
        admission.reset()
        admission.down(1, 100f, 400f, 2000)
        admission.down(2, 300f, 400f, 2300)
        assertFalse(admission.reserved)
    }

    @Test fun twoFreshFingersPinchAndScaleFromTheirOwnDistance() {
        val admission = PinchAdmission()
        admission.down(1, 100f, 400f, 3000)
        admission.down(2, 300f, 400f, 3040)
        assertTrue(admission.allowed)
        // The first sample anchors the pinch.
        assertNull(admission.scale())
        admission.move(1, 50f, 400f)
        admission.move(2, 350f, 400f)
        assertEquals(1.5f, admission.scale()!!, 0.0001f)
        admission.up(1)
        assertNull(admission.scale())
        assertTrue(admission.reserved)
        admission.up(2)
        assertFalse(admission.reserved)
    }

    @Test fun aStickFingerLeftOutLetsTwoOthersPinch() {
        // The host never admits the stick's pointer (0), so the two
        // pinching fingers are this class's only contacts.
        val admission = PinchAdmission()
        admission.down(1, 200f, 300f, 5000)
        admission.down(2, 260f, 300f, 5030)
        assertTrue(admission.allowed)
        assertFalse(admission.contains(0))
    }
}
