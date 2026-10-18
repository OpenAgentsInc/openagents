// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.voice

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import org.webrtc.AudioSource
import org.webrtc.AudioTrack
import org.webrtc.DataChannel
import org.webrtc.MediaConstraints
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.SdpObserver
import org.webrtc.SessionDescription
import org.webrtc.audio.JavaAudioDeviceModule
import java.util.concurrent.Executors
import java.util.concurrent.ExecutorService
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

/** The RTC calls the request driver needs; tests can replace this boundary without native code. */
internal interface AndroidVoiceRtcSession : AutoCloseable {
    fun createOffer(complete: (Result<String>) -> Unit)
    fun applyAnswer(sdp: String, complete: (Result<Unit>) -> Unit)
    fun setMuted(muted: Boolean)
    fun levels(complete: (Result<Pair<UShort, UShort>>) -> Unit)
}

/** Creates one RTC peer for a call. */
internal fun interface AndroidVoiceRtcSessionFactory {
    fun create(onConnectionEnded: () -> Unit): AndroidVoiceRtcSession
}

/** Handles Android media requests with one serialized worker and a call-scoped RTC peer. */
class AndroidWebRtcMediaDriver internal constructor(
    private val hasMicrophonePermission: () -> Boolean,
    private val createSession: AndroidVoiceRtcSessionFactory,
    private val audioMonitor: AndroidVoiceAudioMonitor? = null,
    private val worker: ExecutorService = Executors.newSingleThreadExecutor(),
) : VoiceMediaDriver {
    constructor(context: Context) : this(
        hasMicrophonePermission = {
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.RECORD_AUDIO,
            ) == PackageManager.PERMISSION_GRANTED
        },
        createSession = AndroidVoiceRtcSessionFactory { onConnectionEnded ->
            AndroidWebRtcVoiceRtcSession.create(context.applicationContext, onConnectionEnded)
        },
        audioMonitor = AndroidVoiceAudioMonitor(context.applicationContext),
    )

    @Volatile private var eventListener: ((VoiceMediaPlatformEvent) -> Unit)? = null
    @Volatile private var closed = false
    private var session: AndroidVoiceRtcSession? = null

    override fun execute(request: uniffi.openagents_core.VoiceMediaRequest, complete: (VoiceMediaResult) -> Unit) {
        if (closed) {
            complete(VoiceMediaResult(failure = uniffi.openagents_core.VoiceMediaFailure.UNAVAILABLE))
            return
        }
        try {
            worker.execute {
                if (closed) {
                    complete(unavailable())
                    return@execute
                }
                when (request.operation) {
                    uniffi.openagents_core.VoiceMediaOperation.PREPARE -> prepare(complete)
                    uniffi.openagents_core.VoiceMediaOperation.OFFER -> withSession(complete) { current ->
                        current.createOffer { result ->
                            complete(result.fold(
                                onSuccess = { VoiceMediaResult(sdp = it) },
                                onFailure = { unavailable() },
                            ))
                        }
                    }
                    uniffi.openagents_core.VoiceMediaOperation.APPLY_ANSWER -> withSession(complete) { current ->
                        val sdp = request.sdp
                        if (sdp.isNullOrBlank() || sdp.length > MAX_SDP_LENGTH) {
                            complete(unavailable())
                        } else {
                            current.applyAnswer(sdp) { result ->
                                complete(result.fold(
                                    onSuccess = { VoiceMediaResult() },
                                    onFailure = { unavailable() },
                                ))
                            }
                        }
                    }
                    uniffi.openagents_core.VoiceMediaOperation.SET_MUTED -> withSession(complete) { current ->
                        runCatching { current.setMuted(request.muted) }
                            .onSuccess { complete(VoiceMediaResult()) }
                            .onFailure { complete(unavailable()) }
                    }
                    uniffi.openagents_core.VoiceMediaOperation.LEVELS -> withSession(complete) { current ->
                        current.levels { result ->
                            complete(result.fold(
                                onSuccess = { VoiceMediaResult(microphone = it.first, speaker = it.second) },
                                onFailure = { unavailable() },
                            ))
                        }
                    }
                    uniffi.openagents_core.VoiceMediaOperation.CLOSE -> close()
                }
            }
        } catch (_: RejectedExecutionException) {
            complete(unavailable())
        }
    }

    private fun prepare(complete: (VoiceMediaResult) -> Unit) {
        if (!hasMicrophonePermission()) {
            complete(VoiceMediaResult(failure = uniffi.openagents_core.VoiceMediaFailure.PERMISSION_DENIED))
            return
        }
        runCatching {
            if (session == null) {
                session = createSession.create {
                    eventListener?.invoke(VoiceMediaPlatformEvent.ConnectionEnded)
                }
            }
            audioMonitor?.start { event -> eventListener?.invoke(event) }
        }.onSuccess { complete(VoiceMediaResult()) }
            .onFailure { complete(unavailable()) }
    }

    private inline fun withSession(
        complete: (VoiceMediaResult) -> Unit,
        action: (AndroidVoiceRtcSession) -> Unit,
    ) {
        val current = session
        if (closed || current == null) {
            complete(unavailable())
        } else {
            action(current)
        }
    }

    override fun setEventListener(listener: ((VoiceMediaPlatformEvent) -> Unit)?) {
        eventListener = listener
    }

    @Synchronized
    override fun close() {
        if (closed) return
        closed = true
        eventListener = null
        val dispose = {
            runCatching { audioMonitor?.close() }
            runCatching { session?.close() }
            session = null
        }
        try {
            worker.execute { dispose() }
        } catch (_: RejectedExecutionException) {
            dispose()
        }
        worker.shutdown()
    }

    private fun unavailable() =
        VoiceMediaResult(failure = uniffi.openagents_core.VoiceMediaFailure.UNAVAILABLE)

    private companion object {
        const val MAX_SDP_LENGTH = 65_536
    }
}

/** Android WebRTC peer implementation; Rust continues to own signaling and call state. */
private class AndroidWebRtcVoiceRtcSession private constructor(
    private val peer: PeerConnection,
    private val audioSource: AudioSource,
    private val audioTrack: AudioTrack,
    private val dataChannel: DataChannel,
    private val onConnectionEnded: () -> Unit,
) : AndroidVoiceRtcSession {
    @Volatile private var closed = false
    @Volatile private var offerCompletion: ((Result<String>) -> Unit)? = null
    @Volatile private var answerCompletion: ((Result<Unit>) -> Unit)? = null
    @Volatile private var answerSet = false
    @Volatile private var connectionReady = false
    @Volatile private var channelReady = false
    private val timeoutWorker: ScheduledExecutorService = Executors.newSingleThreadScheduledExecutor()

    init {
        dataChannel.registerObserver(object : DataChannel.Observer {
            override fun onBufferedAmountChange(previousAmount: Long) = Unit

            override fun onStateChange() {
                channelReady = dataChannel.state() == DataChannel.State.OPEN
                if (dataChannel.state() == DataChannel.State.CLOSED) onConnectionEnded()
                finishAnswerWhenReady()
            }

            override fun onMessage(buffer: DataChannel.Buffer) = Unit
        })
        channelReady = dataChannel.state() == DataChannel.State.OPEN
    }

    override fun createOffer(complete: (Result<String>) -> Unit) {
        if (closed || offerCompletion != null) {
            complete(Result.failure(IllegalStateException("RTC peer is not ready")))
            return
        }
        offerCompletion = complete
        timeoutWorker.schedule({ finishOffer(Result.failure(IllegalStateException("ICE gathering timed out"))) }, 30, TimeUnit.SECONDS)
        peer.createOffer(sdpObserver, MediaConstraints())
    }

    override fun applyAnswer(sdp: String, complete: (Result<Unit>) -> Unit) {
        if (closed || answerCompletion != null) {
            complete(Result.failure(IllegalStateException("RTC peer is not ready")))
            return
        }
        answerCompletion = complete
        timeoutWorker.schedule({ finishAnswer(Result.failure(IllegalStateException("RTC connection timed out"))) }, 30, TimeUnit.SECONDS)
        peer.setRemoteDescription(
            sdpObserver,
            SessionDescription(SessionDescription.Type.ANSWER, sdp),
        )
    }

    override fun setMuted(muted: Boolean) {
        check(!closed) { "RTC peer is closed" }
        audioTrack.setEnabled(!muted)
    }

    fun onIceGatheringComplete() {
        val description = peer.localDescription()?.description
        finishOffer(description?.let { Result.success(it) }
            ?: Result.failure(IllegalStateException("Local offer is missing")))
    }

    fun onConnectionStateChanged(state: String) {
        when (state) {
            "CONNECTED" -> {
                connectionReady = true
                finishAnswerWhenReady()
            }
            "DISCONNECTED", "FAILED", "CLOSED" -> {
                connectionReady = false
                finishAnswer(Result.failure(IllegalStateException("RTC connection ended")))
            }
        }
    }

    override fun levels(complete: (Result<Pair<UShort, UShort>>) -> Unit) {
        if (closed) {
            complete(Result.failure(IllegalStateException("RTC peer is closed")))
            return
        }
        peer.getStats { report ->
            var microphone = 0.0
            var speaker = 0.0
            report.statsMap.values.forEach { stat ->
                val level = (stat.members["audioLevel"] as? Number)?.toDouble() ?: return@forEach
                when (stat.type) {
                    "media-source" -> microphone = maxOf(microphone, level)
                    "inbound-rtp" -> speaker = maxOf(speaker, level)
                }
            }
            complete(Result.success(microphone.toUShortLevel() to speaker.toUShortLevel()))
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        finishOffer(Result.failure(IllegalStateException("RTC peer is closed")))
        finishAnswer(Result.failure(IllegalStateException("RTC peer is closed")))
        audioTrack.setEnabled(false)
        dataChannel.close()
        dataChannel.unregisterObserver()
        peer.close()
        peer.dispose()
        audioTrack.dispose()
        audioSource.dispose()
        timeoutWorker.shutdownNow()
    }

    private val sdpObserver = object : SdpObserver {
        override fun onCreateSuccess(description: SessionDescription) {
            if (closed) return
            peer.setLocalDescription(this, description)
        }

        override fun onSetSuccess() {
            if (peer.iceGatheringState() == PeerConnection.IceGatheringState.COMPLETE) {
                finishOffer(peer.localDescription()?.description?.let { Result.success(it) }
                    ?: Result.failure(IllegalStateException("Local offer is missing")))
            }
            if (answerCompletion != null) {
                answerSet = true
                finishAnswerWhenReady()
            }
        }

        override fun onCreateFailure(error: String) {
            finishOffer(Result.failure(IllegalStateException(error)))
        }

        override fun onSetFailure(error: String) {
            finishOffer(Result.failure(IllegalStateException(error)))
            finishAnswer(Result.failure(IllegalStateException(error)))
        }
    }

    private fun finishOffer(result: Result<String>) {
        val completion = offerCompletion ?: return
        offerCompletion = null
        if (!closed && result.isSuccess && result.getOrNull().orEmpty().length > MAX_SDP_LENGTH) {
            completion(Result.failure(IllegalStateException("Offer exceeds the SDP limit")))
        } else {
            completion(result)
        }
    }

    private fun finishAnswer(result: Result<Unit>) {
        val completion = answerCompletion ?: return
        answerCompletion = null
        completion(result)
    }

    private fun finishAnswerWhenReady() {
        if (answerSet && connectionReady && channelReady) {
            answerSet = false
            finishAnswer(Result.success(Unit))
        }
    }

    private fun Double.toUShortLevel(): UShort =
        (coerceIn(0.0, 1.0) * UShort.MAX_VALUE.toDouble()).toInt().toUShort()

    private companion object {
        const val MAX_SDP_LENGTH = 65_536
        private val initializeLock = Any()
        @Volatile private var initialized = false
        @Volatile private var sharedFactory: PeerConnectionFactory? = null

        fun create(context: Context, onConnectionEnded: () -> Unit): AndroidWebRtcVoiceRtcSession {
            val factory = synchronized(initializeLock) {
                sharedFactory ?: run {
                    if (!initialized) {
                        PeerConnectionFactory.initialize(
                            PeerConnectionFactory.InitializationOptions.builder(context)
                                .createInitializationOptions(),
                        )
                        initialized = true
                    }
                    val audio = JavaAudioDeviceModule.builder(context).createAudioDeviceModule()
                    PeerConnectionFactory.builder()
                        .setAudioDeviceModule(audio)
                        .createPeerConnectionFactory()
                        .also { sharedFactory = it }
                }
            }
            val audioSource = factory.createAudioSource(MediaConstraints())
            val audioTrack = factory.createAudioTrack("voice-audio", audioSource).apply {
                setEnabled(false)
            }
            var rtcSession: AndroidWebRtcVoiceRtcSession? = null
            val peer = requireNotNull(
                factory.createPeerConnection(
                    PeerConnection.RTCConfiguration(emptyList()),
                    AndroidWebRtcPeerObserver.create(onConnectionEnded) { state ->
                        rtcSession?.let { session ->
                            if (state == "ICE_GATHERING_COMPLETE") session.onIceGatheringComplete()
                            else session.onConnectionStateChanged(state)
                        }
                    },
                ),
            ) { "Could not create a WebRTC peer connection" }
            peer.addTrack(audioTrack, listOf("voice"))
            val channel = requireNotNull(
                peer.createDataChannel("oai-events", DataChannel.Init().apply { ordered = true }),
            ) { "Could not create the voice event channel" }
            return AndroidWebRtcVoiceRtcSession(peer, audioSource, audioTrack, channel, onConnectionEnded).also {
                rtcSession = it
            }
        }
    }
}

/** Uses a dynamic proxy so optional observer methods can vary across maintained SDK releases. */
private object AndroidWebRtcPeerObserver {
    fun create(
        onConnectionEnded: () -> Unit,
        onState: (String) -> Unit,
    ): PeerConnection.Observer = java.lang.reflect.Proxy.newProxyInstance(
        PeerConnection.Observer::class.java.classLoader,
        arrayOf(PeerConnection.Observer::class.java),
    ) { _, method, args ->
        if (method.name == "onIceGatheringChange" && args?.firstOrNull()?.toString() == "COMPLETE") {
            onState("ICE_GATHERING_COMPLETE")
        } else if (method.name == "onIceConnectionChange" || method.name == "onConnectionChange") {
            val state = args?.firstOrNull()?.toString().orEmpty()
            onState(state)
            when (state) {
                "DISCONNECTED", "FAILED", "CLOSED" -> onConnectionEnded()
            }
        }
        null
    } as PeerConnection.Observer
}
