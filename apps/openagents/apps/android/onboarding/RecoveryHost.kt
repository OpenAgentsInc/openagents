package com.openagents.app.onboarding

import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import android.view.Window
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.CoreAndroidRecovery
import uniffi.openagents_core.RecoveryFrame

interface RecoveryPort : AutoCloseable {
    fun start(device: String): RecoveryFrame
    fun render(): RecoveryFrame
    fun acknowledge(instance: String, revision: Long)
    fun activate(instance: String, revision: Long, key: String): RecoveryFrame
    fun change(instance: String, revision: Long, key: String, value: String)
    fun authenticated(token: ULong, accepted: Boolean): RecoveryFrame
    fun redact()
}
class CoreRecoveryPort(private val core: CoreAndroidRecovery, private val ownsCore: Boolean = true) : RecoveryPort {
    override fun start(device: String) = core.start(device)
    override fun render() = core.render()
    override fun acknowledge(instance: String, revision: Long) = core.acknowledge(instance, revision.toULong())
    override fun activate(instance: String, revision: Long, key: String) = core.activate(instance, revision.toULong(), key)
    override fun change(instance: String, revision: Long, key: String, value: String) = core.change(instance, revision.toULong(), key, value)
    override fun authenticated(token: ULong, accepted: Boolean) = core.authenticated(token, accepted)
    override fun redact() = core.redact()
    override fun close() { if (ownsCore) { core.shutdown(); core.destroy() } else core.redact() }
}

/** A screen, not an Activity. Supply one bounded serial worker owned by the Android shell. */
class RecoveryHost(
    private val parent: ViewGroup,
    window: Window,
    private val port: RecoveryPort,
    private val authentication: DeviceAuthentication,
    private val clipboard: SecretClipboard,
    private val worker: (() -> Unit) -> Unit,
    private val openChat: () -> Unit,
    private val semantics: (() -> com.openagents.app.accessibility.NativeSemantics)? = null,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private val screen = SecretScreen(window, parent)
    private var adapter: SemanticViewAdapter? = null
    private var generation = 0L
    private var closed = false
    private var foreground = true
    private var authenticating: ULong? = null
    private var authHandle: AutoCloseable? = null
    private var pendingResult: Pair<ULong, Boolean>? = null
    private var timeout: Runnable? = null

    private fun ui() { check(Looper.myLooper() == Looper.getMainLooper()) }
    /** Call after the Rust account has A3's stores. Existing identities skip creation in Rust. */
    fun start(device: String) { ui(); dispatch { port.start(device) } }
    fun refresh() { ui(); dispatch { port.render() } }
    private fun dispatch(operation: () -> RecoveryFrame) {
        if (closed) return
        val stamp = generation
        worker {
            try {
                val frame = operation()
                main.post { if (!closed && stamp == generation && foreground) mount(frame) }
            } catch (_: Exception) {
                // Do not log SDK/storage exceptions or values from secret operations.
                main.post { if (!closed && stamp == generation) background() }
            }
        }
    }
    private fun mount(frame: RecoveryFrame) {
        ui()
        if (frame.secure) screen.protect()
        val renderer = adapter ?: SemanticViewAdapter(parent.context,
            { instance, revision, key -> dispatch { port.activate(instance, revision, key) } },
            { instance, revision, key, value -> worker { port.change(instance, revision, key, value) } },
            { instance, revision -> port.acknowledge(instance, revision) }, semantics = semantics?.invoke()).also { adapter = it }
        try {
            renderer.mount(parent, frame.json)
            if (frame.secure) screen.harden()
            screen.show()
            frame.clipboard?.let { clipboard.copy(it.text, it.clearAfterMs.toLong()) }
            val request = frame.authentication
            if (request != null && authenticating != request.token) {
                authenticating = request.token
                val token = request.token
                val deadline = Runnable { if (!closed && authenticating == token) cancelAuthentication() }
                timeout = deadline
                main.postDelayed(deadline, request.timeoutMs.toLong())
                val handle = authentication.authenticate(request.title, request.description) { accepted ->
                    main.post {
                        if (!closed && authenticating == token) {
                            pendingResult = token to accepted
                            deliverAuthentication()
                        }
                    }
                }
                if (authenticating == token) authHandle = handle else handle.close()
            }
            if (frame.openChat) openChat()
        } catch (_: Exception) { background() }
    }
    private fun deliverAuthentication() {
        if (!foreground || closed) return
        val result = pendingResult ?: return
        if (authenticating != result.first) return
        pendingResult = null; authenticating = null
        timeout?.let { main.removeCallbacks(it) }; timeout = null
        authHandle?.close(); authHandle = null
        dispatch { port.authenticated(result.first, result.second) }
    }
    private fun cancelAuthentication() {
        authenticating = null; pendingResult = null
        timeout?.let { main.removeCallbacks(it) }; timeout = null
        authHandle?.close(); authHandle = null
        worker { port.redact() }
        if (foreground && !closed) refresh()
    }
    /** Forward onPause before the Activity loses clipboard access, and before a task preview. */
    fun background() {
        ui(); if (closed) return
        foreground = false; generation++
        try { clipboard.close() } catch (_: Exception) { } finally {
            adapter?.dispose(); adapter = null
            screen.hide(); parent.removeAllViews()
        }
        // A device-credential Activity may pause this Activity. Keep only its pending token,
        // never a word screen; its result must wait for foreground and Rust's original deadline.
        if (authenticating == null) worker { port.redact() }
    }
    fun foreground() {
        ui(); if (closed) return
        foreground = true
        if (pendingResult != null) deliverAuthentication() else if (authenticating == null) refresh()
    }
    override fun close() {
        ui(); if (closed) return
        background(); closed = true; generation++
        cancelAuthentication(); main.removeCallbacksAndMessages(null)
        clipboard.close(); screen.close()
        worker { port.close() }
    }
}
