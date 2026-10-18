// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import uniffi.openagents_core.VoiceCall
import uniffi.openagents_core.VoiceAudioFocus as CoreVoiceAudioFocus
import uniffi.openagents_core.VoiceAudioRoute as CoreVoiceAudioRoute
import uniffi.openagents_core.VoiceMediaFailure
import uniffi.openagents_core.VoiceMediaListener
import uniffi.openagents_core.VoiceMediaOperation
import uniffi.openagents_core.VoiceMediaRequest
import uniffi.openagents_core.VoiceNetworkState as CoreVoiceNetworkState
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

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

    data class ProximityChanged(val near: Boolean) : VoiceMediaPlatformEvent

    data object ConnectionEnded : VoiceMediaPlatformEvent
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
    // Serializes callbacks with close without holding the state monitor across callbacks.
    private val callbackGate = ReentrantLock()
    private val waitingForBinding = ArrayDeque<VoiceMediaRequest>()
    private val inFlight = mutableSetOf<ULong>()
    private data class Binding(
        val generation: Long,
        val completer: VoiceMediaCompleter,
        val stateSink: ((VoiceMediaPlatformEvent) -> Unit)?,
    )

    private var generation = 0L
    private var binding: Binding? = null
    private var closed = false

    private fun isActiveLocked(current: Binding): Boolean =
        !closed && generation == current.generation && binding === current

    init {
        driver.setEventListener(::receivePlatformEvent)
    }

    /** Binds the returned UniFFI call handle and drains any requests received during startup. */
    fun bind(call: VoiceCall): Boolean = bind(
        VoiceMediaCompleter { requestId, failure, sdp, microphone, speaker ->
            call.completeMedia(requestId, failure, sdp, microphone, speaker)
        },
    ) { event ->
        when (event) {
            is VoiceMediaPlatformEvent.RouteChanged -> call.updateAudioRoute(
                when (event.route) {
                    VoiceAudioRoute.EARPIECE -> CoreVoiceAudioRoute.EARPIECE
                    VoiceAudioRoute.SPEAKER -> CoreVoiceAudioRoute.SPEAKER
                    VoiceAudioRoute.WIRED_HEADSET -> CoreVoiceAudioRoute.WIRED_HEADSET
                    VoiceAudioRoute.BLUETOOTH -> CoreVoiceAudioRoute.BLUETOOTH
                    VoiceAudioRoute.UNKNOWN -> CoreVoiceAudioRoute.UNKNOWN
                },
            )
            is VoiceMediaPlatformEvent.AudioFocusChanged -> call.updateAudioFocus(
                when (event.state) {
                    VoiceAudioFocusState.GAINED -> CoreVoiceAudioFocus.GAINED
                    VoiceAudioFocusState.LOST -> CoreVoiceAudioFocus.LOST
                    VoiceAudioFocusState.LOST_TRANSIENT -> CoreVoiceAudioFocus.LOST_TRANSIENT
                    VoiceAudioFocusState.LOST_TRANSIENT_CAN_DUCK ->
                        CoreVoiceAudioFocus.LOST_TRANSIENT_CAN_DUCK
                },
            )
            is VoiceMediaPlatformEvent.NetworkChanged -> call.updateNetworkState(
                when (event.state) {
                    VoiceNetworkState.AVAILABLE -> CoreVoiceNetworkState.AVAILABLE
                    VoiceNetworkState.LOST -> CoreVoiceNetworkState.LOST
                },
            )
            is VoiceMediaPlatformEvent.ProximityChanged -> call.updateProximity(event.near)
            VoiceMediaPlatformEvent.ConnectionEnded -> call.stop()
        }
        platformEventHandler(event)
    }

    /** Binding separately keeps lifecycle behavior testable without a live UniFFI handle. */
    fun bind(completer: VoiceMediaCompleter): Boolean {
        return bind(completer, null)
    }

    internal fun bind(
        completer: VoiceMediaCompleter,
        stateSink: ((VoiceMediaPlatformEvent) -> Unit)?,
    ): Boolean {
        val (current, waiting) = synchronized(lock) {
            if (closed || binding != null) return false
            generation += 1
            val current = Binding(generation, completer, stateSink)
            binding = current
            current to waitingForBinding.toList().also { waitingForBinding.clear() }
        }
        waiting.forEach { dispatch(it, current) }
        return true
    }

    override fun `onRequest`(request: VoiceMediaRequest) {
        if (request.operation == VoiceMediaOperation.CLOSE) {
            close()
            return
        }

        val ready = synchronized(lock) {
            if (closed || !inFlight.add(request.requestId)) return
            val current = binding
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

    private fun dispatch(request: VoiceMediaRequest, current: Binding) {
        val active = synchronized(lock) { isActiveLocked(current) }
        if (!active) return
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
        current: Binding,
        result: VoiceMediaResult,
    ) {
        callbackGate.withLock {
            synchronized(lock) {
                if (!isActiveLocked(current) || !inFlight.remove(request.requestId)) return
            }
            current.completer.complete(
                request.requestId,
                result.failure,
                result.sdp,
                result.microphone,
                result.speaker,
            )
        }
    }

    private fun receivePlatformEvent(event: VoiceMediaPlatformEvent) {
        val current = synchronized(lock) {
            if (closed) return
            binding
        }
        callbackGate.withLock {
            val active = synchronized(lock) {
                !closed && (current == null || isActiveLocked(current))
            }
            if (!active) return
            platformEventHandler(event)
            val sink = current?.stateSink ?: return
            val stillActive = synchronized(lock) { isActiveLocked(current) }
            if (stillActive) sink(event)
        }
    }

    override fun close() {
        callbackGate.withLock {
            val shouldCloseDriver = synchronized(lock) {
                if (closed) return
                closed = true
                generation += 1
                binding = null
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
}
