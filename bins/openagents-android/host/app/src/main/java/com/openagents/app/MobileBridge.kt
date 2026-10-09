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
class MobileBridge(private val context: Context, private val computersFixture: Boolean = false,
                   private val walletFixture: Boolean = false, private val chatFixture: Boolean = false,
                   private val gymFixture: Boolean = false, private val accountOrigin: String? = null,
                   private val changed: () -> Unit) {
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
    /** A packet asked for after Rust said it changed is on its way, and whether Rust changed again since. */
    private var changeInFlight = false
    private var changedAgain = false

    /** The latest app packet (`openagents.mobile.v1`). */
    var packet: JSONObject? = null; private set
    /** The Wi-Fi multicast lock nearby pairing needs while it listens. */
    private val nearby = NearbyMulticast(context)
    /** The open terminal's latest view. */
    var terminalView: JSONObject? = null; private set
    var failure: String? = null; private set
    var pending = 0; private set
    val busy get() = pending > 0
    /** Counts the Coder tab's requests to open Account > Computers. */
    var computersRequested = 0; private set
    /** The Coder tab's last request to open another screen (wallet, keys, playtest, report), and how many so far. */
    var screenRequested: String? = null; private set
    var screenRequests = 0; private set
    /** Counts the Chat tab's requests for the photo picker (`pick_image`), only while the chat takes images. */
    var imagePicks = 0; private set
    /** Decoded images for the chat's `image:` surfaces, by resource. */
    private val images = HashMap<String, android.graphics.Bitmap>()

    init {
        pending += 1
        worker.execute {
            val result = runCatching {
                val config = json("state_dir" to DeviceKey.stateDirectory(context).path,
                    "secret_hex" to DeviceKey.loadOrCreate(context),
                    // This host draws the Computers list and its navigation.
                    "native_computers" to true,
                    // Its transcript painter reads chat rows from Rust.
                    "pulled_transcripts" to true,
                    // It draws the shell (#11126): the top bar, the drawer, the cards.
                    "shell" to true,
                    // The chat router's context names the build.
                    "app_build" to "${ReportDevice.version} (${ReportDevice.build})")
                // The iroh key beside the device key: connecting a computer
                // dials it over iroh. Without it, pairing uses the relay only.
                runCatching { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.IROH) }
                    .onSuccess { config.put("iroh_secret_hex", it) }
                // iroh's DNS resolver reads Android's settings through JNI.
                OpenAgentsNative.installContext(context.applicationContext)
                // Debug builds only: Coder's offline Computers fixture, which
                // contacts no host or relay.
                if (computersFixture) config.put("computers_fixture", true)
                // Debug builds only: an offline wallet with no money.
                if (walletFixture) config.put("wallet_fixture", true)
                // Debug builds only: an offline chat worker that sends the chat router's fields.
                if (chatFixture) config.put("chat_fixture", true)
                // Debug builds only: the Gym's recorded cards and a recorded test run.
                if (gymFixture) config.put("gym_fixture", true)
                // Debug builds only: sign in to another site (staging).
                accountOrigin?.let { config.put("account_origin", it) }
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
        gymWorld()
        providerKeysLoad()
        linkHello()
        watchChanges()
    }

    /**
     * Hands Rust the trainer's Verse world key: it names the trainer on the
     * menu, reads their XP, and signs their test requests. Rust keeps it in
     * memory only.
     */
    fun gymWorld() {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        send(json("op" to "gym_world", "world_secret_hex" to secret))
    }

    // Account > Your keys (BYOK, #10176)

    private val keyPrefs get() = context.getSharedPreferences("provider_keys", Context.MODE_PRIVATE)
    /** Keys the person entered that wait for their test, by provider; in memory only. */
    private val pendingKeys = HashMap<String, String>()
    /** Rust holds the saved keys: from here on its switch is the one saved. */
    private var providerKeysLoaded = false
    /** Counts the asks for "Use your keys for everything?" after a key was added. */
    var askMineRequests = 0; private set
    /** A key could not be saved in the Keystore. */
    var providerKeyError: String? = null; private set

    /** Hands Rust the person's own provider keys and the saved switch; Rust keeps them in memory only. */
    fun providerKeysLoad() {
        val keys = try { DeviceKey.providerKeys(context) } catch (_: Exception) { org.json.JSONArray() }
        send(json("op" to "provider_keys", "keys" to keys, "mine" to keyPrefs.getBoolean("mine", false))) {
            providerKeysLoaded = true
        }
    }

    /** A key from the secure field: Rust tests it, and it is kept only once the provider accepts it. */
    fun providerKeyAdd(provider: String, key: String) {
        val trimmed = key.trim()
        if (trimmed.isEmpty()) return
        pendingKeys[provider] = trimmed
        send(json("op" to "provider_key_add", "provider" to provider, "key" to trimmed))
    }

    fun providerKeyTest(provider: String) = send(json("op" to "provider_key_test", "provider" to provider))

    fun providerKeyRemove(provider: String) {
        DeviceKey.deleteProviderKey(context, provider)
        send(json("op" to "provider_key_remove", "provider" to provider))
    }

    fun providerKeysMine(on: Boolean) = send(json("op" to "provider_keys_mine", "on" to on))

    /** Keeps or drops the keys whose tests finished, and saves the switch. */
    private fun settleProviderKeys(state: JSONObject?) {
        state ?: return
        for (done in state.optJSONArray("done")?.objects() ?: emptyList()) {
            val provider = done.optString("provider")
            val key = pendingKeys.remove(provider) ?: continue
            if (done.optBoolean("kept")) {
                providerKeyError = if (DeviceKey.saveProviderKey(context, provider, key)) null
                    else "Could not save the key on this phone. It works until OpenAgents restarts."
                if (done.optBoolean("ask_mine")) askMineRequests += 1
            }
        }
        val mine = state.optBoolean("mine")
        if (providerKeysLoaded && keyPrefs.getBoolean("mine", false) != mine) keyPrefs.edit().putBoolean("mine", mine).apply()
    }

    /** A tap on a Gym button, by the ID Rust gave it. */
    fun gym(id: String) = send(json("op" to "gym", "id" to id))

    /** Train Coder, from the Verse's Gym board or Account: Rust opts into the Gym and asks for the Chat tab (`coder_go` is `chat`). */
    fun gymTrain() = send(json("op" to "gym_train"))

    /** Profile, from Account: Rust shows the Profile sheet on the Chat tab (`coder_go` is `chat`). */
    fun profile() = send(json("op" to "profile"))

    /**
     * A shell action (`coder_tab::ShellAction`, #11126): the Chat / Code
     * switch, the drawer, a recent chat, See all, or a card's Try it.
     */
    fun shell(action: String, vararg fields: Pair<String, Any?>) =
        send(json("op" to "shell", "shell" to json("action" to action, *fields)))

    /** An action on the account surface (`account_link::Action`, #11107). */
    fun link(action: String, vararg fields: Pair<String, Any?>) =
        send(json("op" to "link", "link" to json("action" to action, *fields)))

    /** Tells Rust this phone's name and the session the Keystore-encrypted store kept. */
    private fun linkHello() {
        val name = runCatching {
            android.provider.Settings.Global.getString(context.contentResolver, android.provider.Settings.Global.DEVICE_NAME)
        }.getOrNull()?.takeIf { it.isNotBlank() } ?: android.os.Build.MODEL
        val session = runCatching { DeviceKey.loadAccountSession(context) }.getOrNull()
        link("hello", "name" to name, "session" to session)
    }

    /** Text Rust asked the system share sheet to open, until it shows. */
    var gymShare: String? = null

    /**
     * Rust says when its packet changes (a transcript page, a streamed reply,
     * a computer's task summary): a thread of its own waits on it and asks for
     * the packet at once, instead of on a timer.
     */
    private fun watchChanges() {
        Thread({
            var seen = 0L
            while (!disposed) {
                val now = try { OpenAgentsNative.waitChange(seen, 30_000) } catch (problem: Throwable) { break }
                if (now == seen) continue
                seen = now
                main.post { rustChanged() }
            }
        }, "openagents-changes").apply { isDaemon = true }.start()
    }

    /** Asks for the changed packet one at a time; a change while one is on its way asks again when it arrives. */
    private fun rustChanged() {
        if (disposed) return
        if (changeInFlight) { changedAgain = true; return }
        changeInFlight = true
        changedAgain = false
        send(json("op" to "changed")) {
            changeInFlight = false
            if (changedAgain) rustChanged()
        }
    }

    /** The Coder tab shows or hides; while it shows a live chat, Rust asks for a packet every second. */
    fun coderShown(shown: Boolean) = OpenAgentsNative.coderShown(shown)

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

    // The theme (#11028)

    /** The appearance last reported, so an unchanged one is not sent again. */
    private var reportedNight: Boolean? = null

    /** The phone's appearance (the night bits of uiMode), which the System theme follows. */
    fun systemAppearance(night: Boolean) {
        if (night == reportedNight) return
        reportedNight = night
        send(json("op" to "system_appearance", "dark" to night))
    }

    /** Account > Appearance: `system`, `light`, or `dark`. Rust saves it. */
    fun theme(choice: String) = send(json("op" to "theme", "theme" to choice))

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

    /**
     * The trainer card for the Verse world key (`openagents.trainer.v1`).
     * With `reveal`, the answer also carries that key's nsec: ask only after
     * the person chose to see it. Nothing here logs it.
     */
    fun trainer(reveal: Boolean = false, preview: Boolean = false, received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        call(json("op" to "trainer", "world_secret_hex" to secret, "reveal" to reveal, "preview" to preview)) { text ->
            text?.let { runCatching { JSONObject(it) }.getOrNull() }
                ?.takeIf { it.optString("schema") == "openagents.trainer.v1" }?.let(received)
        }
    }

    /**
     * Publishes the trainer profile after the person confirms: `shown` puts
     * their level over their head in other players' Grids. The answer is
     * the trainer packet.
     */
    fun trainerProfile(shown: Boolean, received: (JSONObject) -> Unit) = trainerCall(json("op" to "trainer_profile", "shown" to shown), received)

    /**
     * Sets the display name shown over the player's head in the Verse; an
     * empty name clears it. The answer is the account packet with the name
     * as the app cleaned it, which is also kept for the Verse surface.
     */
    fun setDisplayName(name: String, received: (JSONObject) -> Unit) = call(json("op" to "set_display_name", "name" to name)) { text ->
        text?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.takeIf { it.optString("schema") == "openagents.account.v1" }?.let { packet ->
                VerseSurface.saveDisplayName(context, packet.optString("display_name").ifEmpty { null })
                received(packet)
            }
    }

    /** Adds a key (npub or hex) to, or removes one from, the trainer profile, and publishes it. */
    fun trainerLink(add: String? = null, remove: String? = null, received: (JSONObject) -> Unit) =
        trainerCall(json("op" to "trainer_link", "add" to add, "remove" to remove), received)

    /**
     * Signs the trainer card and publishes it at its address, after the
     * person confirms (`openagents.trainer-card.v1`: the JSON, its file
     * name, and its link, or an error).
     */
    fun trainerExport(received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        call(json("op" to "trainer_export", "world_secret_hex" to secret)) { reply(it, "openagents.trainer-card.v1")?.let(received) }
    }

    private fun trainerCall(request: JSONObject, received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        call(request.put("world_secret_hex", secret)) { text ->
            reply(text, "openagents.trainer.v1")?.takeIf { it.has("npub") }?.let(received)
        }
    }

    /** The Report a problem form for `tab` and `route` (`openagents.report-draft.v1`). */
    fun reportDraft(tab: String, route: String, received: (JSONObject) -> Unit) =
        call(json("op" to "report_draft", "tab" to tab, "route" to route)) { reply(it, "openagents.report-draft.v1")?.let(received) }

    /**
     * Files a report signed by the Verse world key. Rust checks every field,
     * refuses a screenshot from the Wallet or a key screen, and seals it to
     * the triage key; the answer is My reports (`openagents.reports.v1`).
     */
    fun sendReport(form: JSONObject, received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        call(json("op" to "report_send", "world_secret_hex" to secret, "form" to form)) { reply(it, "openagents.reports.v1")?.let(received) }
    }

    /**
     * Give feedback on selected text (#10127), signed by the Verse world key.
     * Rust adds where the text came from and seals it to the triage key; the
     * answer's `feedback` is what the dialog says.
     */
    fun sendFeedback(form: JSONObject, received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { return }
        call(json("op" to "feedback_send", "world_secret_hex" to secret, "form" to form)) { reply(it, "openagents.reports.v1")?.let(received) }
    }

    /** My reports; with the world key, reports that wait or failed are sent again. */
    fun reports(received: (JSONObject) -> Unit) {
        val secret = try { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) } catch (_: Exception) { "" }
        call(json("op" to "reports", "world_secret_hex" to secret)) { reply(it, "openagents.reports.v1")?.let(received) }
    }

    /** Deletes the playtest log; logging goes on recording. */
    fun playtestClear(received: (JSONObject) -> Unit) =
        call(json("op" to "playtest_clear")) { reply(it, "openagents.reports.v1")?.let(received) }

    /** Where the tester is; Rust records it unless this build turned playtest logging off. */
    fun playtestScreen(tab: String, route: String) = send(json("op" to "playtest_screen", "tab" to tab, "route" to route))

    private fun reply(text: String?, schema: String): JSONObject? =
        text?.let { runCatching { JSONObject(it) }.getOrNull() }?.takeIf { it.optString("schema") == schema }

    /**
     * An agent payment request's answer: `spend_approve` or `spend_deny`
     * with `request`, `spend_block` or `spend_allow` with `host`, or
     * `spend_dismiss`. Rust checks every field.
     */
    fun spend(op: String, vararg fields: Pair<String, Any?>) = send(json("op" to op, *fields))

    /**
     * The saved unilateral-exit backup, for a file export the person asked
     * for: its file name and text, or why there is none. It arrives in
     * Rust's direct reply, never in the app packet.
     */
    fun walletExitExport(received: (String?, String?, String?) -> Unit) = call(json("op" to "wallet_exit_export")) { text ->
        val reply = text?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.takeIf { it.optString("schema") == "openagents.wallet-file.v1" }
        received(reply?.textOrNull("file_name"), reply?.textOrNull("text"), reply?.textOrNull("error"))
    }

    /** Open a computer from the native Computers list. */
    fun openComputer(host: String) = send(json("op" to "computers_open", "host" to host))
    /** A choice from a Computers row's menu, already confirmed when it asks. */
    fun chooseComputer(host: String, choice: String) = send(json("op" to "computers_choose", "host" to host, "choice" to choice))
    /** `home`, `add`, `activity`, `owner_key`, `keep_directory`, or `refresh`. */
    fun computersGo(destination: String) = send(json("op" to "computers_go", "to" to destination))
    /** Connect a computer: open the scanner, hand Rust a code, or close. */
    fun connectOpen() = send(json("op" to "connect_open"))
    fun connectCode(value: String) = send(json("op" to "connect_code", "value" to value))
    /** The app was opened with a connect link (the desktop app's QR code,
     *  read by the system camera): show Connect a computer and pair with it. */
    fun connectLink(value: String) = send(json("op" to "connect_link", "value" to value))
    fun connectClose() = send(json("op" to "connect_close"))
    fun connectNearby(id: String) = send(json("op" to "connect_nearby", "id" to id))
    fun refreshComputers() = send(json("op" to "computers_refresh"))
    fun snapshot() = send(json("op" to "snapshot"))

    /**
     * The paired computer Everglade's studio acts through: the first one the
     * Computers list shows online, as its host key and name.
     */
    fun studioComputer(): Pair<String, String>? {
        val rows = packet?.objectOrNull("computers_home")?.optJSONArray("rows") ?: return null
        return rows.objects().firstOrNull { it.optString("tone") == "online" }
            ?.let { it.getString("host") to it.optString("name") }
    }

    /**
     * Takes the app's link to the paired computer [host] for the studio, on
     * the worker, and hands [done] its token on the main thread (null when
     * the app is not running). The Verse surface connects with it.
     */
    fun studioLinks(host: String, done: (Long?) -> Unit) {
        if (disposed) return done(null)
        worker.execute {
            val token = runCatching {
                check(handle != 0L) { failure ?: "OpenAgents has not started." }
                OpenAgentsNative.studioLinks(handle, host)
            }.getOrNull()
            main.post { if (!disposed) done(token) }
        }
    }

    /** An activation on a surface: `computers`, `coder`, `tailnet`, or `terminal`. */
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
        nearby.hold(false)
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

    /** Whether the chat takes images; off since #10093, when the phone became text only. */
    val attachmentsEnabled get() = PhoneAttachments.enabled(packet)

    /**
     * Attaches a photo's encoded bytes to the open chat's draft; Rust decodes
     * and bounds them. Dropped here while the chat is text only (#10093).
     */
    fun attachImage(name: String, bytes: ByteArray) {
        if (disposed || !attachmentsEnabled) return
        pending += 1
        worker.execute {
            val result = runCatching {
                check(handle != 0L) { failure ?: "OpenAgents has not started." }
                OpenAgentsNative.attachImage(handle, name, bytes)
            }
            main.post {
                pending -= 1
                if (disposed) return@post
                result.exceptionOrNull()?.let { if (handle != 0L) failure = it.message }
                receive(result.getOrNull())
            }
        }
    }

    /** The image an `image:` surface shows: the bytes Rust decoded and bounded, decoded here for display. */
    fun image(resource: String, received: (android.graphics.Bitmap?) -> Unit) {
        images[resource]?.let { return received(it) }
        if (disposed) return received(null)
        worker.execute {
            val bitmap = runCatching {
                check(handle != 0L)
                val bytes = OpenAgentsNative.image(handle, resource)
                if (bytes.isEmpty()) null else {
                    val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
                    var sample = 1
                    while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= 480) sample *= 2
                    android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size,
                        android.graphics.BitmapFactory.Options().apply { inSampleSize = sample })
                }
            }.getOrNull()
            main.post {
                if (bitmap != null) { if (images.size > 16) images.clear(); images[resource] = bitmap }
                received(bitmap)
            }
        }
    }

    private fun send(request: JSONObject, done: (() -> Unit)? = null) {
        call(request) { text ->
            done?.invoke()
            receive(text)
        }
    }

    private fun receive(text: String?) {
        if (text == null) return
        val next = try { packet(text, "openagents.mobile.v1") } catch (problem: Exception) {
            failure = problem.message ?: "OpenAgents returned an unreadable screen."
            changed(); return
        }
        packet = next
        failure = null
        settleProviderKeys(next.objectOrNull("provider_keys"))
        next.objectOrNull("link")?.let { link ->
            AccountLink.settle(context, link)
            link.textOrNull("open_url")?.let { browse(it) }
        }
        nearby.hold(next.optBoolean("nearby_listening"))
        next.objectOrNull("gym")?.textOrNull("share")?.let { gymShare = it }
        when (val go = next.textOrNull("coder_go")) {
            "computers" -> computersRequested += 1
            "pick_image" -> if (PhoneAttachments.pickRequested(next)) imagePicks += 1
            "wallet", "keys", "playtest", "report", "verse_gym", "chat" -> { screenRequested = go; screenRequests += 1 }
        }
        if (!next.optBoolean("terminal")) { terminalView = null; terminalRevision = 0 }
        next.textOrNull("open_url")?.let { link -> open(link) }
        next.textOrNull("wallet_open_url")?.let { link -> browse(link) }
        changed()
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
