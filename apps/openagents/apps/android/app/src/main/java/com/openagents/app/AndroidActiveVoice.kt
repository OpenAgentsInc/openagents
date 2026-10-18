package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import com.openagents.app.platform.*
import com.openagents.app.voice.AndroidVoiceMediaBridge
import com.openagents.app.voice.AndroidWebRtcMediaDriver
import uniffi.openagents_core.*

/** Process-owned platform resources for one Rust-owned call; Activity mounts only borrow them. */
internal class AndroidActiveVoice(
    context: Context,
    private val owner: CoreAndroidHost,
    val generation: ULong,
    private val changed: () -> Unit,
    private val ended: (AndroidActiveVoice) -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var closed = false
    private var endReason: VoiceEndReason? = null
    private val driver = AndroidWebRtcMediaDriver(context)
    private val bridge = AndroidVoiceMediaBridge(driver)
    private val service = AndroidVoiceCallHost(context, object : AndroidVoiceCallHostMount {
        override fun mountCapsule(viewJson: String, call: VoiceCall) = Unit
        override fun mountCallScreen(viewJson: String, call: VoiceCall) = Unit
        override fun mountVoiceSettings(pickerJson: String, usageJson: String, picker: CoreVoiceAgent, usage: CoreVoiceUsage) = Unit
        override fun dismissCallViews() = Unit
    })
    var call: VoiceCall? = null
        private set
    var state: VoiceCallState? = null
        private set
    private val listener = object : VoiceSessionListener {
        override fun onVoiceState(state: VoiceCallState) { main.post {
            if (closed) return@post
            this@AndroidActiveVoice.state = state
            try {
                if (call == null) return@post
                service.update(state)
                changed()
            } catch (_: Exception) { close() }
        } }
        override fun onVoiceClosed(reason: VoiceEndReason?) { main.post { endReason = reason; close() } }
    }
    fun start(contentRevision: ULong) {
        check(!closed && call == null)
        try {
            val started = owner.startHostVoice(generation, contentRevision, bridge, listener)
            call = started
            check(bridge.bind(started))
            service.attach(started)
        } catch (error: Exception) { close(); throw error }
    }
    override fun close() {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (closed) return
        closed = true
        main.removeCallbacksAndMessages(null)
        val current = call
        call = null; state = null
        try { current?.stop() } finally {
            bridge.close()
            service.close()
            if (current != null) runCatching { owner.voiceEnded(generation, current, endReason) }
            ended(this)
            changed()
        }
    }
}
