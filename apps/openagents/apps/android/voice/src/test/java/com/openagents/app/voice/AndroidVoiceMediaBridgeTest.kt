// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import uniffi.openagents_core.VoiceMediaFailure
import uniffi.openagents_core.VoiceMediaOperation
import uniffi.openagents_core.VoiceMediaRequest

class AndroidVoiceMediaBridgeTest {
    @Test
    fun buffersStartupRequestsUntilRustReturnsTheCallHandle() {
        val driver = FakeVoiceMediaDriver()
        val bridge = AndroidVoiceMediaBridge(driver)
        val completed = mutableListOf<ULong>()

        bridge.`onRequest`(request(1u))
        assertTrue(driver.requests.isEmpty())
        assertTrue(bridge.bind(VoiceMediaCompleter { requestId, _, _, _, _ -> completed += requestId }))
        assertEquals(listOf(1uL), driver.requests.map { it.requestId })

        driver.reply(1uL, VoiceMediaResult(sdp = "fixture-offer"))

        assertEquals(listOf(1uL), completed)
        assertFalse(bridge.bind(VoiceMediaCompleter { _, _, _, _, _ -> error("second bind") }))
    }

    @Test
    fun ignoresDuplicateAndLateRepliesAfterClose() {
        val driver = FakeVoiceMediaDriver()
        val bridge = AndroidVoiceMediaBridge(driver)
        val completed = mutableListOf<ULong>()
        bridge.bind(VoiceMediaCompleter { requestId, _, _, _, _ -> completed += requestId })
        bridge.`onRequest`(request(7u))

        val result = VoiceMediaResult(microphone = 12u, speaker = 34u)
        driver.reply(7uL, result)
        driver.reply(7uL, result)
        bridge.close()
        driver.reply(7uL, result)
        bridge.close()

        assertEquals(listOf(7uL), completed)
        assertEquals(1, driver.closeCount)
    }

    @Test
    fun forwardsPermissionDenialFromFakeMedia() {
        val driver = FakeVoiceMediaDriver()
        val bridge = AndroidVoiceMediaBridge(driver)
        val failures = mutableListOf<VoiceMediaFailure?>()
        bridge.bind(VoiceMediaCompleter { _, failure, _, _, _ -> failures += failure })
        bridge.`onRequest`(request(2u, VoiceMediaOperation.PREPARE))

        driver.reply(2uL, VoiceMediaResult(failure = VoiceMediaFailure.PERMISSION_DENIED))

        assertEquals(listOf(VoiceMediaFailure.PERMISSION_DENIED), failures)
        assertEquals(1, driver.requests.size)
    }

    private fun request(
        requestId: ULong,
        operation: VoiceMediaOperation = VoiceMediaOperation.OFFER,
    ) = VoiceMediaRequest(requestId, operation, null, true)

    private class FakeVoiceMediaDriver : VoiceMediaDriver {
        val requests = mutableListOf<VoiceMediaRequest>()
        private val callbacks = mutableMapOf<ULong, (VoiceMediaResult) -> Unit>()
        var closeCount = 0
            private set

        override fun execute(request: VoiceMediaRequest, complete: (VoiceMediaResult) -> Unit) {
            requests += request
            callbacks[request.requestId] = complete
        }

        fun reply(requestId: ULong, result: VoiceMediaResult) {
            callbacks[requestId]?.invoke(result)
        }

        override fun close() {
            closeCount += 1
        }
    }
}
