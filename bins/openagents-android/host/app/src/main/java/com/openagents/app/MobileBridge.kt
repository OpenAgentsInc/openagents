package com.openagents.app

import android.content.ActivityNotFoundException
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import java.util.concurrent.Executors

/**
 * Forwards requests to the Rust app on one serial worker and publishes its
 * packets on the main thread. Rust owns every screen, grant, and connection;
 * this bridge only opens URLs Rust names and collects values Rust asks for.
 */
class MobileBridge(private val context: Context, private val changed: () -> Unit) {
    companion object {
        // Rust keeps each app handle on the thread that created it, so one
        // process-wide worker owns every handle for its whole lifetime.
        private val worker = Executors.newSingleThreadExecutor { Thread(it, "openagents-rust") }
    }

    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L
    private var disposed = false
    private var terminalRevision = 0L
    private var terminalPolling = false

    /** The latest app packet (`openagents.mobile.v1`). */
    var packet: JSONObject? = null; private set
    /** The open terminal's latest view. */
    var terminalView: JSONObject? = null; private set
    var failure: String? = null; private set
    var pending = 0; private set
    val busy get() = pending > 0

    init {
        pending += 1
        worker.execute {
            val result = runCatching {
                val config = json("state_dir" to DeviceKey.stateDirectory(context).path,
                    "secret_hex" to DeviceKey.loadOrCreate(context))
                handle = OpenAgentsNative.create(config.toString())
                check(handle != 0L) { "OpenAgents could not start." }
            }
            main.post {
                pending -= 1
                result.exceptionOrNull()?.let { failure = it.message ?: "OpenAgents could not start." }
                changed()
            }
        }
        send(json("op" to "snapshot"))
    }

    fun lifecycle(active: Boolean) = send(json("op" to "lifecycle", "active" to active))
    fun refreshComputers() = send(json("op" to "computers_refresh"))
    fun snapshot() = send(json("op" to "snapshot"))

    /** An activation on a surface: `computers`, `coder`, `chats`, `tailnet`, or `terminal`. */
    fun activate(surface: String, view: JSONObject, node: String) =
        send(json("op" to "${surface}_activate", "instance" to view.getString("instance"),
            "revision" to view.getLong("revision"), "node" to node))

    /** Answers an input request, or a composer's send, on a surface. */
    fun submit(surface: String, token: String, value: String) =
        send(json("op" to "${surface}_input", "token" to token, "value" to value))

    fun cancel(surface: String, token: String) = send(json("op" to "${surface}_cancel", "token" to token))

    /** A terminal request: a resize, typed text, a key, or a paste. */
    fun terminal(request: JSONObject) = call(request) { receiveTerminal(it) }

    /** Polls the open terminal; skipped while a poll is in flight. */
    fun pollTerminal() {
        if (terminalPolling) return
        terminalPolling = true
        call(json("op" to "terminal_poll", "known" to terminalRevision)) {
            terminalPolling = false
            receiveTerminal(it)
        }
    }

    fun dispose() {
        disposed = true
        worker.execute { if (handle != 0L) { runCatching { OpenAgentsNative.destroy(handle) }; handle = 0 } }
    }

    private fun receiveTerminal(text: String?) {
        val next = text?.let { runCatching { packet(it, "coder.mobile.terminal.v1") }.getOrNull() } ?: return
        if (!next.optBoolean("open")) {
            terminalView = null; terminalRevision = 0; changed(); return
        }
        val view = next.objectOrNull("view")
        val revision = next.optLong("revision")
        if (view != null && revision > terminalRevision) {
            terminalRevision = revision
            terminalView = view
            changed()
        }
        if (next.optBoolean("paste")) {
            val clipboard = context.getSystemService(ClipboardManager::class.java)
            val text = clipboard?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
            terminal(json("op" to "terminal_paste", "text" to (text ?: "")))
        }
    }

    private fun send(request: JSONObject) {
        call(request) { text ->
            if (text == null) return@call
            val next = try { packet(text, "openagents.mobile.v1") } catch (problem: Exception) {
                failure = problem.message ?: "OpenAgents returned an unreadable screen."
                changed(); return@call
            }
            packet = next
            failure = null
            if (!next.optBoolean("terminal")) { terminalView = null; terminalRevision = 0 }
            next.textOrNull("open_url")?.let { link -> open(link) }
            changed()
        }
    }

    /** Opens a sign-in page Rust named, then tells Rust to wait for approval. */
    private fun open(link: String) {
        val uri = Uri.parse(link)
        if (uri.scheme != "https") return
        try {
            context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            send(json("op" to "tailnet_wait_for_sign_in"))
        } catch (_: ActivityNotFoundException) {
            failure = "No browser is available to open the Tailscale sign-in page."
            changed()
        }
    }

    private fun call(request: JSONObject, received: (String?) -> Unit) {
        if (disposed) return
        val encoded = request.toString()
        if (encoded.toByteArray().size > 131_072) { failure = "That request is too large."; changed(); return }
        pending += 1
        worker.execute {
            val result = runCatching {
                check(handle != 0L) { failure ?: "OpenAgents has not started." }
                OpenAgentsNative.call(handle, encoded)
            }
            main.post {
                pending -= 1
                if (disposed) return@post
                result.exceptionOrNull()?.let { if (handle != 0L) failure = it.message }
                received(result.getOrNull())
            }
        }
    }
}
