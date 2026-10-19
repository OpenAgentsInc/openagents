// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.*
import android.net.*
import android.os.*
import kotlinx.coroutines.*
import org.webrtc.*
import uniffi.openagents_core.*
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** One bridge per Rust call. All Android and WebRTC callbacks return to main. */
class AndroidVoiceMedia(
    context: Context,
    private val requestMicrophone: suspend () -> Boolean,
) : VoiceMediaListener, AutoCloseable {
    private val context = context.applicationContext
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val audio = context.getSystemService(AudioManager::class.java)
    private val network = context.getSystemService(ConnectivityManager::class.java)
    private val handler = Handler(Looper.getMainLooper())
    private var call: VoiceCall? = null
    private var closed = false
    private var prepared = false
    private var oldMode = AudioManager.MODE_NORMAL
    private var near = false
    private var speaker = false
    private var factory: PeerConnectionFactory? = null
    private var device: org.webrtc.audio.JavaAudioDeviceModule? = null
    private var source: AudioSource? = null
    private var track: org.webrtc.AudioTrack? = null
    private var peer: PeerConnection? = null
    private var gathering: CancellableContinuation<Unit>? = null
    private val jobs = mutableMapOf<ULong, Job>()
    private val focus = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
        .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build())
        .setOnAudioFocusChangeListener({ if (it < 0) fail() }, handler).build()
    private val devices = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = route()
        override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = route()
    }
    private val networks = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) { handler.post { if (!closed) peer?.restartIce() } }
        // Rust owns reconnect/signaling. Do not construct a second offer here.
    }

    fun bind(call: VoiceCall) { check(this.call == null && !closed); this.call = call }

    override fun onRequest(request: VoiceMediaRequest) {
        handler.post {
            if (request.operation == VoiceMediaOperation.CLOSE) { close(); return@post }
            if (closed) return@post
            if (jobs.size >= 4 || jobs.containsKey(request.requestId)) { fail(); return@post }
            jobs[request.requestId] = scope.launch(start = CoroutineStart.LAZY) {
                try {
                    var sdp: String? = null
                    when (request.operation) {
                        VoiceMediaOperation.PREPARE -> prepare()
                        VoiceMediaOperation.OFFER -> sdp = offer()
                        VoiceMediaOperation.APPLY_ANSWER -> setDescription(false,
                            SessionDescription(SessionDescription.Type.ANSWER, requireNotNull(request.sdp)))
                        VoiceMediaOperation.SET_MUTED -> requireNotNull(track).setEnabled(!request.muted)
                        VoiceMediaOperation.LEVELS -> Unit
                        VoiceMediaOperation.CLOSE -> Unit
                    }
                    if (!closed) call?.completeMedia(request.requestId, null, sdp, 0u, 0u)
                } catch (cancel: CancellationException) { throw cancel
                } catch (error: Exception) {
                    if (!closed) call?.completeMedia(request.requestId,
                        if (error is SecurityException) VoiceMediaFailure.PERMISSION_DENIED else VoiceMediaFailure.UNAVAILABLE,
                        null, 0u, 0u)
                } finally { jobs.remove(request.requestId) }
            }
            jobs[request.requestId]?.start()
        }
    }

    private suspend fun prepare() {
        check(!prepared)
        if (context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED &&
            !requestMicrophone()) throw SecurityException("Microphone permission denied")
        currentCoroutineContext().ensureActive()
        check(!closed)
        oldMode = audio.mode
        prepared = true
        check(audio.requestAudioFocus(focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED)
        audio.mode = AudioManager.MODE_IN_COMMUNICATION
        audio.registerAudioDeviceCallback(devices, handler)
        network.registerDefaultNetworkCallback(networks)
        route()
        PeerConnectionFactory.initialize(PeerConnectionFactory.InitializationOptions.builder(context).createInitializationOptions())
        device = org.webrtc.audio.JavaAudioDeviceModule.builder(context).createAudioDeviceModule()
        factory = PeerConnectionFactory.builder().setAudioDeviceModule(device).createPeerConnectionFactory()
        source = factory!!.createAudioSource(MediaConstraints())
        track = factory!!.createAudioTrack("microphone", source).also { it.setEnabled(false) }
        val config = PeerConnection.RTCConfiguration(emptyList()).apply {
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
        }
        peer = requireNotNull(factory!!.createPeerConnection(config, object : PeerConnection.Observer {
            override fun onSignalingChange(state: PeerConnection.SignalingState) {}
            override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {
                if (state == PeerConnection.IceConnectionState.FAILED) handler.post { fail() }
            }
            override fun onIceConnectionReceivingChange(receiving: Boolean) {}
            override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) {
                if (state == PeerConnection.IceGatheringState.COMPLETE) handler.post {
                    gathering?.let { gathering = null; if (it.isActive) it.resume(Unit) }
                }
            }
            override fun onIceCandidate(candidate: IceCandidate) {}
            override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>) {}
            override fun onAddStream(stream: MediaStream) {}
            override fun onRemoveStream(stream: MediaStream) {}
            override fun onDataChannel(channel: DataChannel) { channel.dispose() }
            override fun onRenegotiationNeeded() {}
            override fun onAddTrack(receiver: RtpReceiver, streams: Array<out MediaStream>) {}
        }))
        peer!!.addTrack(requireNotNull(track), listOf("voice"))
    }

    private suspend fun offer(): String = withTimeout(20_000) {
        val pc = requireNotNull(peer)
        val description = suspendCancellableCoroutine<SessionDescription> { continuation ->
            pc.createOffer(object : DescriptionObserver() {
                override fun onCreateSuccess(sdp: SessionDescription) { handler.post {
                    if (!closed && continuation.isActive) continuation.resume(sdp)
                } }
                override fun onCreateFailure(error: String) { handler.post {
                    if (continuation.isActive) continuation.resumeWithException(IllegalStateException(error))
                } }
            }, MediaConstraints())
        }
        setDescription(true, description)
        suspendCancellableCoroutine<Unit> { continuation ->
            if (pc.iceGatheringState() == PeerConnection.IceGatheringState.COMPLETE) continuation.resume(Unit)
            else gathering = continuation
        }
        pc.localDescription.description
    }

    private suspend fun setDescription(local: Boolean, sdp: SessionDescription) =
        suspendCancellableCoroutine<Unit> { continuation ->
            val observer = object : DescriptionObserver() {
                override fun onSetSuccess() { handler.post {
                    if (!closed && continuation.isActive) continuation.resume(Unit)
                } }
                override fun onSetFailure(error: String) { handler.post {
                    if (continuation.isActive) continuation.resumeWithException(IllegalStateException(error))
                } }
            }
            if (local) requireNotNull(peer).setLocalDescription(observer, sdp)
            else requireNotNull(peer).setRemoteDescription(observer, sdp)
        }

    /** The Activity forwards proximity and speaker selection; external devices win. */
    fun setNearEar(value: Boolean) { near = value; route() }
    fun setSpeaker(value: Boolean) { speaker = value; route() }
    private fun route() {
        if (closed || !prepared) return
        // API 31+: Android handles wired, USB, hearing aids and Bluetooth communication devices.
        val available = audio.availableCommunicationDevices
        val external = available.firstOrNull { it.type in listOf(AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
            AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_USB_HEADSET) }
        val type = if (speaker && !near) AudioDeviceInfo.TYPE_BUILTIN_SPEAKER else AudioDeviceInfo.TYPE_BUILTIN_EARPIECE
        (external ?: available.firstOrNull { it.type == type })?.let { audio.setCommunicationDevice(it) }
    }
    /** Call for Activity destruction, sign-out, or an interruption requiring call termination. */
    fun interrupted() = fail()
    private fun fail() { if (!closed) { val owner = call; close(); owner?.stop() } }
    override fun close() {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (closed) return
        closed = true
        scope.cancel()
        jobs.clear()
        gathering = null
        peer?.close(); peer?.dispose(); peer = null
        track?.dispose(); track = null
        source?.dispose(); source = null
        factory?.dispose(); factory = null
        device?.release(); device = null
        if (prepared) {
            audio.unregisterAudioDeviceCallback(devices)
            runCatching { network.unregisterNetworkCallback(networks) }
            audio.clearCommunicationDevice()
            audio.abandonAudioFocusRequest(focus)
            audio.mode = oldMode
        }
        call = null
    }
    private open class DescriptionObserver : SdpObserver {
        override fun onCreateSuccess(sdp: SessionDescription) {}
        override fun onCreateFailure(error: String) {}
        override fun onSetSuccess() {}
        override fun onSetFailure(error: String) {}
    }
}
