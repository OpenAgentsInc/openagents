// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
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

        assertEquals(listOf<VoiceMediaFailure?>(VoiceMediaFailure.PERMISSION_DENIED), failures)
        assertEquals(1, driver.requests.size)
    }

    @Test
    fun closeCancelsPendingPermissionRequestAndFencesItsReply() {
        val driver = FakeVoiceMediaDriver()
        val bridge = AndroidVoiceMediaBridge(driver)
        val completed = mutableListOf<ULong>()
        bridge.bind(VoiceMediaCompleter { requestId, _, _, _, _ -> completed += requestId })
        bridge.`onRequest`(request(3u, VoiceMediaOperation.PREPARE))

        bridge.`onRequest`(request(4u, VoiceMediaOperation.CLOSE))
        driver.reply(3uL, VoiceMediaResult(failure = VoiceMediaFailure.PERMISSION_DENIED))

        assertTrue(completed.isEmpty())
        assertEquals(1, driver.closeCount)
    }

    @Test
    fun closeDoesNotWaitOnAnExternalCompletionAndFencesLaterReplies() {
        val driver = FakeVoiceMediaDriver()
        val bridge = AndroidVoiceMediaBridge(driver)
        val callbackEntered = CountDownLatch(1)
        val allowCallbackToFinish = CountDownLatch(1)
        val closeReturned = CountDownLatch(1)
        val completed = mutableListOf<ULong>()
        bridge.bind(
            VoiceMediaCompleter { requestId, _, _, _, _ ->
                callbackEntered.countDown()
                assertTrue(allowCallbackToFinish.await(2, TimeUnit.SECONDS))
                completed += requestId
            },
        )
        bridge.`onRequest`(request(6u))

        val callback = Thread { driver.reply(6uL, VoiceMediaResult(sdp = "fixture-answer")) }
        callback.start()
        assertTrue(callbackEntered.await(2, TimeUnit.SECONDS))
        val close = Thread {
            bridge.close()
            closeReturned.countDown()
        }
        close.start()

        assertTrue(closeReturned.await(2, TimeUnit.SECONDS))
        allowCallbackToFinish.countDown()
        callback.join(2_000)
        close.join(2_000)
        driver.reply(6uL, VoiceMediaResult(sdp = "late-answer"))

        assertFalse(callback.isAlive)
        assertFalse(close.isAlive)
        assertEquals(0L, closeReturned.count)
        assertEquals(listOf(6uL), completed)
        assertEquals(1, driver.closeCount)
    }

    @Test
    fun separateCallsKeepCompletionsAndCloseStateIndependent() {
        val firstDriver = FakeVoiceMediaDriver()
        val secondDriver = FakeVoiceMediaDriver()
        val firstBridge = AndroidVoiceMediaBridge(firstDriver)
        val secondBridge = AndroidVoiceMediaBridge(secondDriver)
        val completed = mutableListOf<ULong>()
        val completer = VoiceMediaCompleter { requestId, _, _, _, _ -> completed += requestId }
        firstBridge.bind(completer)
        secondBridge.bind(completer)
        firstBridge.`onRequest`(request(8u))
        secondBridge.`onRequest`(request(9u))

        firstBridge.close()
        firstDriver.reply(8uL, VoiceMediaResult(sdp = "late-first-offer"))
        secondDriver.reply(9uL, VoiceMediaResult(sdp = "second-offer"))

        assertEquals(listOf(9uL), completed)
        assertEquals(1, firstDriver.closeCount)
        assertEquals(0, secondDriver.closeCount)
        secondBridge.close()
        assertEquals(1, secondDriver.closeCount)
    }

    @Test
    fun forwardsTypedRouteFocusAndNetworkEventsAndFencesThemAfterClose() {
        val driver = FakeVoiceMediaDriver()
        val events = mutableListOf<VoiceMediaPlatformEvent>()
        val bridge = AndroidVoiceMediaBridge(driver, events::add)

        driver.emit(VoiceMediaPlatformEvent.RouteChanged(VoiceAudioRoute.BLUETOOTH))
        driver.emit(VoiceMediaPlatformEvent.AudioFocusChanged(VoiceAudioFocusState.LOST_TRANSIENT))
        driver.emit(VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.LOST))
        driver.emit(VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.AVAILABLE))
        driver.emit(VoiceMediaPlatformEvent.ProximityChanged(near = true))
        val afterRecovery = events.toList()
        bridge.close()
        driver.emitLate(VoiceMediaPlatformEvent.RouteChanged(VoiceAudioRoute.SPEAKER))

        assertEquals(
            listOf(
                VoiceMediaPlatformEvent.RouteChanged(VoiceAudioRoute.BLUETOOTH),
                VoiceMediaPlatformEvent.AudioFocusChanged(VoiceAudioFocusState.LOST_TRANSIENT),
                VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.LOST),
                VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.AVAILABLE),
                VoiceMediaPlatformEvent.ProximityChanged(near = true),
            ),
            afterRecovery,
        )
        assertEquals(afterRecovery, events)
    }

    @Test
    fun callbacksCanWaitForAnotherThreadToReenterTheBridge() {
        lateinit var bridge: AndroidVoiceMediaBridge
        val driver = FakeVoiceMediaDriver()
        val callbackFailures = mutableListOf<Throwable>()
        bridge = AndroidVoiceMediaBridge(driver) {
            assertReentrantBridgeCall(callbackFailures) {
                bridge.`onRequest`(request(22u))
            }
        }
        assertTrue(
            bridge.bind(
                VoiceMediaCompleter { _, _, _, _, _ ->
                    assertReentrantBridgeCall(callbackFailures) {
                        bridge.`onRequest`(request(23u))
                    }
                },
                {
                    assertReentrantBridgeCall(callbackFailures) {
                        bridge.`onRequest`(request(24u))
                    }
                },
            ),
        )

        driver.emit(VoiceMediaPlatformEvent.NetworkChanged(VoiceNetworkState.AVAILABLE))
        bridge.`onRequest`(request(21u))
        driver.reply(23uL, VoiceMediaResult(sdp = "fixture-answer"))

        assertTrue(callbackFailures.isEmpty(), "callback could not reenter the bridge: $callbackFailures")
        assertEquals(listOf(22uL, 24uL, 21uL, 23uL), driver.requests.map { it.requestId })
        bridge.close()
    }

    private fun assertReentrantBridgeCall(
        failures: MutableList<Throwable>,
        action: () -> Unit,
    ) {
        val finished = CountDownLatch(1)
        val failure = AtomicReference<Throwable?>()
        Thread {
            try {
                action()
            } catch (error: Throwable) {
                failure.set(error)
            } finally {
                finished.countDown()
            }
        }.start()
        if (!finished.await(2, TimeUnit.SECONDS)) {
            failures += IllegalStateException("bridge monitor blocked callback reentry")
            return
        }
        failure.get()?.let(failures::add)
    }

    private fun request(
        requestId: ULong,
        operation: VoiceMediaOperation = VoiceMediaOperation.OFFER,
    ) = VoiceMediaRequest(requestId, operation, null, true)

    private class FakeVoiceMediaDriver : VoiceMediaDriver {
        val requests = mutableListOf<VoiceMediaRequest>()
        private val callbacks = mutableMapOf<ULong, (VoiceMediaResult) -> Unit>()
        private var eventListener: ((VoiceMediaPlatformEvent) -> Unit)? = null
        private var previousEventListener: ((VoiceMediaPlatformEvent) -> Unit)? = null
        var closeCount = 0
            private set

        override fun execute(request: VoiceMediaRequest, complete: (VoiceMediaResult) -> Unit) {
            requests += request
            callbacks[request.requestId] = complete
        }

        override fun setEventListener(listener: ((VoiceMediaPlatformEvent) -> Unit)?) {
            if (listener != null) previousEventListener = listener
            eventListener = listener
        }

        fun reply(requestId: ULong, result: VoiceMediaResult) {
            callbacks[requestId]?.invoke(result)
        }

        fun emit(event: VoiceMediaPlatformEvent) {
            eventListener?.invoke(event)
        }

        fun emitLate(event: VoiceMediaPlatformEvent) {
            previousEventListener?.invoke(event)
        }

        override fun close() {
            closeCount += 1
        }
    }
}
