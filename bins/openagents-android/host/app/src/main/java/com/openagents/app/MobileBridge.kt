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
class MobileBridge(private val context: Context, private val computersFixture: Boolean = false, private val changed: () -> Unit) {
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
                    "secret_hex" to DeviceKey.loadOrCreate(context),
                    // This host draws the Computers list and its navigation.
                    "native_computers" to true)
                // Debug builds only: Coder's offline Computers fixture, which
                // contacts no host or relay.
                if (computersFixture) config.put("computers_fixture", true)
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

    /**
     * Hands Rust the Spark wallet's seed from the Keystore-encrypted store.
     * Rust ignores a repeat. The seed is never logged or kept here.
     */
    fun openWallet() {
        if (walletOpened) return
        val entropy = try { DeviceKey.loadOrCreateSpark(context) } catch (problem: Exception) {
            failure = problem.message; changed(); return
        }
        walletOpened = true
        send(json("op" to "wallet_open", "entropy_hex" to entropy))
    }
    private var walletOpened = false

    /** A Wallet request whose fields Rust checks. */
    fun wallet(op: String, vararg fields: Pair<String, Any?>) = send(json("op" to op, *fields))

    /**
     * The recovery words, for a dialog the person asked to see after a
     * warning. They arrive in Rust's direct reply, never in the app packet;
     * nothing here keeps or logs them.
     */
    fun walletWords(received: (List<String>) -> Unit) = call(json("op" to "wallet_words")) { text ->
        val reply = text?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.takeIf { it.optString("schema") == "openagents.wallet-secret.v1" } ?: return@call
        reply.optJSONArray("words")?.let { list -> received((0 until list.length()).map { list.getString(it) }) }
    }

    /**
     * Restores from recovery words: Rust checks them, this saves the seed in
     * the Keystore-encrypted store, and Rust replaces the running wallet.
     * `done` gets the reason on failure, never the words.
     */
    fun restoreWallet(words: String, done: (String?) -> Unit) = call(json("op" to "wallet_restore_check", "words" to words)) { text ->
        val reply = text?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.takeIf { it.optString("schema") == "openagents.wallet-secret.v1" }
        val entropy = reply?.textOrNull("entropy_hex")
        if (entropy == null) { done(reply?.textOrNull("error") ?: "The words could not be checked."); return@call }
        try { DeviceKey.replaceSpark(context, entropy) } catch (problem: Exception) {
            done(problem.message ?: "The restored key could not be saved."); return@call
        }
        walletOpened = true
        send(json("op" to "wallet_open", "entropy_hex" to entropy, "replace" to true))
        done(null)
    }

    /**
     * This device's keys and the changelog (`openagents.account.v1`). With
     * `reveal`, the answer also carries the nsec: ask only after the person
     * chose to see it, and keep it no longer than it shows.
     */
    fun account(reveal: Boolean = false, received: (JSONObject) -> Unit) = call(json("op" to "account", "reveal" to reveal)) { text ->
        text?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.takeIf { it.optString("schema") == "openagents.account.v1" }?.let(received)
    }

    /** Open a computer from the native Computers list. */
    fun openComputer(host: String) = send(json("op" to "computers_open", "host" to host))
    /** A choice from a Computers row's menu, already confirmed when it asks. */
    fun chooseComputer(host: String, choice: String) = send(json("op" to "computers_choose", "host" to host, "choice" to choice))
    /** `home`, `add`, `activity`, `owner_key`, `keep_directory`, or `refresh`. */
    fun computersGo(destination: String) = send(json("op" to "computers_go", "to" to destination))
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
            next.textOrNull("wallet_open_url")?.let { link -> browse(link) }
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

    /** Opens a bitcoin purchase page Rust named, once. */
    private fun browse(link: String) {
        val uri = Uri.parse(link)
        if (uri.scheme != "https") return
        try { context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        catch (_: ActivityNotFoundException) { failure = "No browser is available to open this page."; changed() }
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
