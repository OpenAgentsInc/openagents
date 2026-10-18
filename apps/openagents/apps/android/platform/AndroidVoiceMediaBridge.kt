// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import uniffi.openagents_core.VoiceCall
import uniffi.openagents_core.VoiceMediaFailure
import uniffi.openagents_core.VoiceMediaListener
import uniffi.openagents_core.VoiceMediaOperation
import uniffi.openagents_core.VoiceMediaRequest

/** A media result returned to Rust. SDP is present only for offer operations. */
data class VoiceMediaResult(
    val failure: VoiceMediaFailure? = null,
    val sdp: String? = null,
    val microphone: UShort = 0u,
    val speaker: UShort = 0u,
)

/** Platform media work runs on its own executor and never owns call signaling state. */
interface VoiceMediaDriver {
    fun execute(request: VoiceMediaRequest, complete: (VoiceMediaResult) -> Unit)
    fun setEventListener(listener: ((VoiceMediaPlatformEvent) -> Unit)?)
    fun close()
}

/** Android audio routes reported by the platform media layer. */
enum class VoiceAudioRoute {
    EARPIECE,
    SPEAKER,
    WIRED_HEADSET,
    BLUETOOTH,
    UNKNOWN,
}

/** Android audio focus state reported by the platform media layer. */
enum class VoiceAudioFocusState {
    GAINED,
    LOST,
    LOST_TRANSIENT,
    LOST_TRANSIENT_CAN_DUCK,
}

/** Network availability reported by the platform media layer. */
enum class VoiceNetworkState {
    AVAILABLE,
    LOST,
}

/** Platform-only events; call and signaling state remain in Rust. */
sealed interface VoiceMediaPlatformEvent {
    data class RouteChanged(val route: VoiceAudioRoute) : VoiceMediaPlatformEvent

    data class AudioFocusChanged(val state: VoiceAudioFocusState) : VoiceMediaPlatformEvent

    data class NetworkChanged(val state: VoiceNetworkState) : VoiceMediaPlatformEvent
}

/** Completes one Rust-owned media request. */
fun interface VoiceMediaCompleter {
    fun complete(
        requestId: ULong,
        failure: VoiceMediaFailure?,
        sdp: String?,
        microphone: UShort,
        speaker: UShort,
    )
}

/** Adapts Rust media requests to an Android media driver without moving call state into Kotlin. */
class AndroidVoiceMediaBridge(
    private val driver: VoiceMediaDriver,
    private val platformEventHandler: (VoiceMediaPlatformEvent) -> Unit = {},
) : VoiceMediaListener, AutoCloseable {
    private val lock = Any()
    private val waitingForBinding = ArrayDeque<VoiceMediaRequest>()
    private val inFlight = mutableSetOf<ULong>()
    private var completer: VoiceMediaCompleter? = null
    private var closed = false

    init {
        driver.setEventListener(::receivePlatformEvent)
    }

    /** Binds the returned UniFFI call handle and drains any requests received during startup. */
    fun bind(call: VoiceCall): Boolean = bind(
        VoiceMediaCompleter { requestId, failure, sdp, microphone, speaker ->
            call.completeMedia(requestId, failure, sdp, microphone, speaker)
        },
    )

    /** Binding separately keeps lifecycle behavior testable without a live UniFFI handle. */
    fun bind(completer: VoiceMediaCompleter): Boolean {
        val waiting = synchronized(lock) {
            if (closed || this.completer != null) return false
            this.completer = completer
            waitingForBinding.toList().also { waitingForBinding.clear() }
        }
        waiting.forEach(::dispatch)
        return true
    }

    override fun `onRequest`(request: VoiceMediaRequest) {
        if (request.operation == VoiceMediaOperation.CLOSE) {
            close()
            return
        }

        val ready = synchronized(lock) {
            if (closed || !inFlight.add(request.requestId)) return
            val current = completer
            if (current == null) {
                // Rust bounds outstanding requests to four while the call starts.
                check(waitingForBinding.size < 4)
                waitingForBinding.addLast(request)
                null
            } else {
                request to current
            }
        }
        ready?.let { (queued, current) -> dispatch(queued, current) }
    }

    private fun dispatch(request: VoiceMediaRequest) {
        val current = synchronized(lock) {
            if (closed) return
            completer ?: return
        }
        dispatch(request, current)
    }

    private fun dispatch(request: VoiceMediaRequest, current: VoiceMediaCompleter) {
        try {
            driver.execute(request) { result -> complete(request, current, result) }
        } catch (_: Exception) {
            complete(
                request,
                current,
                VoiceMediaResult(failure = VoiceMediaFailure.UNAVAILABLE),
            )
        }
    }

    private fun complete(
        request: VoiceMediaRequest,
        current: VoiceMediaCompleter,
        result: VoiceMediaResult,
    ) {
        synchronized(lock) {
            if (closed || completer !== current || !inFlight.remove(request.requestId)) return
            // Keep close behind the callback so a result cannot arrive after close returns.
            current.complete(
                request.requestId,
                result.failure,
                result.sdp,
                result.microphone,
                result.speaker,
            )
        }
    }

    private fun receivePlatformEvent(event: VoiceMediaPlatformEvent) {
        synchronized(lock) {
            if (closed) return
            platformEventHandler.invoke(event)
        }
    }

    override fun close() {
        val shouldCloseDriver = synchronized(lock) {
            if (closed) return
            closed = true
            waitingForBinding.clear()
            inFlight.clear()
            true
        }
        if (shouldCloseDriver) {
            driver.setEventListener(null)
            driver.close()
        }
    }
}
