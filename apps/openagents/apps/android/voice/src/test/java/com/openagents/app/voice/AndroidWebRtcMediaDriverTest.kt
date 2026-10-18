// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import uniffi.openagents_core.VoiceMediaFailure
import uniffi.openagents_core.VoiceMediaOperation
import uniffi.openagents_core.VoiceMediaRequest

class AndroidWebRtcMediaDriverTest {
    @Test
    fun refusesToPrepareWithoutMicrophonePermission() {
        var factoryCalled = false
        val driver = AndroidWebRtcMediaDriver(
            hasMicrophonePermission = { false },
            createSession = AndroidVoiceRtcSessionFactory { _ ->
                factoryCalled = true
                FakeRtcSession()
            },
        )

        val result = execute(driver, request(1u, VoiceMediaOperation.PREPARE))

        assertEquals(VoiceMediaFailure.PERMISSION_DENIED, result.failure)
        assertFalse(factoryCalled)
        driver.close()
    }

    @Test
    fun mapsRustMediaRequestsToOneCallScopedRtcPeer() {
        val session = FakeRtcSession()
        val driver = AndroidWebRtcMediaDriver(
            hasMicrophonePermission = { true },
            createSession = AndroidVoiceRtcSessionFactory { _ -> session },
        )

        assertEquals(null, execute(driver, request(2u, VoiceMediaOperation.PREPARE)).failure)
        assertEquals("fixture-offer", execute(driver, request(3u, VoiceMediaOperation.OFFER)).sdp)
        assertEquals(null, execute(driver, request(4u, VoiceMediaOperation.APPLY_ANSWER, "fixture-answer")).failure)
        assertEquals(null, execute(driver, request(5u, VoiceMediaOperation.SET_MUTED, muted = true)).failure)
        val levels = execute(driver, request(6u, VoiceMediaOperation.LEVELS))

        assertEquals("fixture-answer", session.answer)
        assertEquals(true, session.muted)
        assertEquals(123u.toUShort(), levels.microphone)
        assertEquals(456u.toUShort(), levels.speaker)

        driver.close()
        assertTrue(session.closed.await(2, TimeUnit.SECONDS))
    }

    private fun execute(
        driver: AndroidWebRtcMediaDriver,
        request: VoiceMediaRequest,
    ): VoiceMediaResult {
        val latch = CountDownLatch(1)
        val result = AtomicReference<VoiceMediaResult>()
        driver.execute(request) {
            result.set(it)
            latch.countDown()
        }
        assertTrue(latch.await(2, TimeUnit.SECONDS), "media request did not complete")
        return result.get()
    }

    private fun request(
        id: ULong,
        operation: VoiceMediaOperation,
        sdp: String? = null,
        muted: Boolean = false,
    ) = VoiceMediaRequest(id, operation, sdp, muted)

    private class FakeRtcSession : AndroidVoiceRtcSession {
        var answer: String? = null
        var muted: Boolean? = null
        val closed = CountDownLatch(1)

        override fun createOffer(complete: (Result<String>) -> Unit) {
            complete(Result.success("fixture-offer"))
        }

        override fun applyAnswer(sdp: String, complete: (Result<Unit>) -> Unit) {
            answer = sdp
            complete(Result.success(Unit))
        }

        override fun setMuted(muted: Boolean) {
            this.muted = muted
        }

        override fun levels(complete: (Result<Pair<UShort, UShort>>) -> Unit) {
            complete(Result.success(123u.toUShort() to 456u.toUShort()))
        }

        override fun close() {
            closed.countDown()
        }
    }
}
