package com.openagents.coder

import org.junit.Assert.*
import org.junit.Test

class PinchAdmissionTest {
    @Test fun movementAndHeldControlsDoNotBecomePinches() {
        val admission = PinchAdmission()
        admission.down(1, 100f, 400f, 1000)
        admission.move(1, 100f, 350f)
        admission.down(2, 300f, 400f, 1050)
        assertFalse(admission.reserved)
        admission.up(2)
        admission.down(3, 300f, 400f, 1200)
        assertFalse(admission.reserved)

        admission.reset()
        admission.down(1, 100f, 400f, 2000)
        admission.down(2, 300f, 400f, 2300)
        assertFalse(admission.reserved)
    }

    @Test fun aFreshPinchKeepsRemainingFingersOutOfMovementUntilReleased() {
        val admission = PinchAdmission()
        admission.down(1, 100f, 400f, 3000)
        admission.down(2, 300f, 400f, 3040)
        assertTrue(admission.reserved)
        assertTrue(admission.allowed)
        admission.up(1)
        assertTrue(admission.reserved)
        assertFalse(admission.allowed)
        admission.up(2)
        assertFalse(admission.reserved)

        admission.down(3, 100f, 400f, 4000)
        assertFalse(admission.reserved)
        admission.down(4, 300f, 400f, 4040)
        assertTrue(admission.allowed)
        admission.reset()
        assertFalse(admission.reserved)
        assertFalse(admission.allowed)
    }
}
