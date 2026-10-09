// The Account tab's own screens: identity keys, this device, the changelog,
// and the native Computers list. Rust derives every key form, owns the
// changelog, and builds every Computers row and choice; these views only
// show, copy, and forward what it returns. They follow the iOS
// `AccountScreens.swift` and the Computers list in `OpenAgentsApp.swift`.
package com.openagents.app

import android.app.AlertDialog
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject

internal class AccountScreens(private val activity: MainActivity, private val bridge: MobileBridge) {
    private fun dialog() = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    /** A section heading above a grouped card, as iOS's inset-grouped lists. */
    private fun LinearLayout.section(title: String?, footer: String? = null, rows: LinearLayout.() -> Unit) {
        title?.let { add(activity.label(it.uppercase(), 13f, Palette.SECONDARY), 20).apply { setPadding(activity.dp(16), 0, 0, 0) } }
        val card = activity.column().apply {
            background = activity.rounded(Palette.SURFACE, 12f)
            setPadding(activity.dp(16), activity.dp(6), activity.dp(16), activity.dp(6))
            rows()
        }
        add(card, if (title == null) 20 else 6)
        footer?.let { add(activity.label(it, 13f, Palette.SECONDARY), 6).apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) } }
    }

    /** A key in monospaced type, broken into 32-character lines so it never hyphenates. */
    private fun key(value: String?, key: String? = null): TextView =
        activity.label(value?.chunked(32)?.joinToString("\n") ?: "Not available yet.", 13f,
            if (value == null) Palette.SECONDARY else Palette.PRIMARY, key = key, mono = true, selectable = value != null).apply {
            setPadding(0, activity.dp(10), 0, activity.dp(10))
            contentDescription = value ?: "Not available yet."
        }

    private fun LinearLayout.rowDivider() = addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))

    /** A copy control that confirms briefly. */
    private fun copyRow(title: String, value: String?, key: String, secret: Boolean = false): View =
        activity.label(title, 16f, Palette.LINK, key = key).apply {
            setPadding(0, activity.dp(12), 0, activity.dp(12))
            enabled(value != null)
            setOnClickListener {
                value ?: return@setOnClickListener
                Clipboard.copy(activity, title, value, secret)
                text = "Copied"
                postDelayed({ text = title }, 1500)
            }
        }

    // Identity keys

    private var account: JSONObject? = null
    /** The nsec, only while the Identity keys screen shows it. */
    var nsec: String? = null; private set

    /**
     * This device's Nostr identity: the npub first, hex beside it, and the
     * nsec only after an explicit reveal and warning.
     */
    fun identity(refresh: () -> Unit): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        val npub = account?.textOrNull("npub")
        val hex = account?.textOrNull("public_hex")
        val shown = account?.textOrNull("display_name")
        body.section("Display name", "Other players in the Grid read this over your avatar. Up to 24 letters, digits, and punctuation.") {
            add(key(shown ?: "Not set", "identity-display-name")); rowDivider()
            add(action("Change display name", "identity-display-name-change") { askDisplayName(shown, refresh) })
        }
        body.section("Public key", "Share your npub freely. It identifies this device.") {
            add(key(npub, "identity-npub")); rowDivider(); add(copyRow("Copy npub", npub, "identity-copy-npub"))
        }
        body.section("Public key (hex)") {
            add(key(hex, "identity-hex")); rowDivider(); add(copyRow("Copy hex", hex, "identity-copy-hex"))
        }
        body.section("Secret key", account?.textOrNull("origin")) {
            val secret = nsec
            if (secret != null) {
                add(key(secret, "identity-nsec").apply { setTextIsSelectable(false) })
                rowDivider()
                add(copyRow("Copy nsec", secret, "identity-copy-nsec", secret = true))
                rowDivider()
                add(activity.label("Hide nsec", 16f, Palette.LINK, key = "identity-hide").apply {
                    setPadding(0, activity.dp(12), 0, activity.dp(12))
                    setOnClickListener { hideNsec(); refresh() }
                })
            } else {
                add(activity.label("•".repeat(24), 13f, Palette.SECONDARY, mono = true).apply {
                    setPadding(0, activity.dp(10), 0, activity.dp(10)); contentDescription = "Hidden"
                })
                rowDivider()
                add(activity.label("Reveal nsec", 16f, Palette.FAILURE, key = "identity-reveal").apply {
                    setPadding(0, activity.dp(12), 0, activity.dp(12))
                    enabled(account != null)
                    setOnClickListener { warn(refresh) }
                })
            }
        }
        return ScrollView(activity).apply { addView(body) }
    }

    private fun askDisplayName(current: String?, refresh: () -> Unit) {
        val field = android.widget.EditText(activity).apply {
            hint = "Shown over your head"; tag = "identity-display-name-field"; isSingleLine = true
            setText(current ?: "")
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_FLAG_CAP_WORDS
            importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
        }
        val frame = android.widget.FrameLayout(activity).apply {
            setPadding(activity.dp(20), activity.dp(8), activity.dp(20), 0); addView(field)
        }
        dialog().setTitle("Display name")
            .setView(frame)
            .setPositiveButton("Save") { _, _ ->
                bridge.setDisplayName(field.text.toString()) { account = it; refresh() }
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** Reads the keys and changelog (never the nsec) for the Account screens. */
    fun load(refresh: () -> Unit) = bridge.account {
        account = it
        VerseSurface.saveDisplayName(activity, it.textOrNull("display_name"))
        refresh()
    }

    private fun warn(refresh: () -> Unit) = reveal("Reveal your nsec?",
        "Anyone with your nsec can act as this device on your computers. Never share it, and make sure no one can see your screen.",
        { done -> bridge.account(reveal = true) { done(it.textOrNull("nsec")) } }, refresh)

    /** Asks first; then fetches the nsec and shows it with screenshots blocked. */
    private fun reveal(title: String, message: String, fetch: ((String?) -> Unit) -> Unit, refresh: () -> Unit) {
        dialog().setTitle(title).setMessage(message)
            .setPositiveButton("Reveal") { _, _ ->
                fetch { secret ->
                    nsec = secret ?: return@fetch
                    if (!(BuildConfig.DEBUG && activity.intent.getBooleanExtra("allow_secret_captures", false))) {
                        activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    }
                    refresh()
                }
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** Drops the nsec: when the screen closes, the app leaves the foreground, or on Hide. */
    fun hideNsec() {
        nsec = null
        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
    }

    // Trainer

    private var card: JSONObject? = null

    fun loadTrainer(preview: Boolean, refresh: () -> Unit) = bridge.trainer(preview = preview) { next ->
        if (next.toString() != card?.toString()) { card = next; refresh() }
    }

    /**
     * The trainer card: the level over this player's head in the Grid, the
     * curve it uses, and the counted awards behind it, derived in Rust from
     * signed NIP-XP awards. The trainer key is the Verse world key; its nsec
     * shows only after an explicit reveal.
     */
    fun trainer(preview: Boolean, refresh: () -> Unit): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        val card = card
        if (card?.optString("state") == "preview") body.add(activity.label(
            "Preview: a labeled fixture of six tutorial reproductions, not real awards.", 13f, 0xFFFFD60A.toInt(), key = "trainer-preview"), 12)
        val state = card?.optString("state")
        body.section(null, if (card != null && state != "ready" && state != "preview") "Reading awards from ${card.optString("relay")}…" else null) {
            if (card == null) { add(activity.label("Reading your XP…", 16f, Palette.SECONDARY).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) }); return@section }
            val level = card.optInt("level"); val xp = card.optLong("xp")
            add(activity.row().apply {
                gravity = Gravity.BOTTOM; setPadding(0, activity.dp(10), 0, 0); tag = "trainer-card"
                addView(activity.label("Level $level", 30f, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
                addView(activity.label("$xp XP", 18f))
            })
            val next = card.optLong("next_level_at").toDouble()
            val start = if (level <= 1) 0.0 else Math.ceil(100 * Math.pow((level - 1).toDouble(), 1.5))
            val progress = if (next > start) ((xp - start) / (next - start)).coerceIn(0.0, 1.0) else 0.0
            add(android.widget.ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply {
                max = 1000; this.progress = (progress * 1000).toInt(); tag = "trainer-progress"
                progressTintList = android.content.res.ColorStateList.valueOf(Palette.PRIMARY)
            }, 8)
            add(activity.label("${card.optLong("to_next")} XP to level ${level + 1} · ${card.optString("curve")}", 13f, Palette.SECONDARY), 4)
            val shown = xp > 0 && card.optString("profile") == "shown"
            add(activity.label("Over your head in the Grid: ${card.optString("tag")}${if (shown) " · lv $level" else ""}", 13f,
                Palette.SECONDARY, mono = true).apply { setPadding(0, 0, 0, activity.dp(10)) }, 2)
        }
        card ?: return ScrollView(activity).apply { addView(body) }
        profileSection(body, card, refresh)
        linkedKeysSection(body, card, refresh)
        exportSection(body, card, refresh)
        val titles = card.optJSONArray("titles").strings()
        if (titles.isNotEmpty()) body.section("Titles") {
            add(activity.label(titles.joinToString(", "), 16f).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) })
        }
        body.section("Counted awards", "Counted by OpenAgents. ${card.optString("note")}") {
            val awards = card.optJSONArray("awards")?.objects() ?: emptyList()
            if (awards.isEmpty()) add(activity.label("No awards yet. Reproduce a published pass from its recipe to earn your first; a tutorial quest is worth 50 XP.",
                14f, Palette.SECONDARY).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) })
            for (award in awards) {
                add(activity.column().apply {
                    setPadding(0, activity.dp(10), 0, activity.dp(10))
                    val top = activity.row()
                    top.addView(activity.label(award.getString("title"), 16f, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
                    top.addView(activity.label("+${award.optLong("xp")} XP", 14f))
                    add(top)
                    add(activity.label("${award.getString("quest")} · ${award.getString("role")} · ${award.getString("season")}", 12f,
                        Palette.SECONDARY, mono = true), 2)
                    val link = award.optString("link")
                    contentDescription = "${award.getString("title")}, ${award.optLong("xp")} XP, opens in the browser"
                    if (link.startsWith("https://")) setOnClickListener { activity.openLink(link) }
                })
                rowDivider()
            }
            val open = card.optInt("open_quests")
            add(activity.label(if (open == 1) "1 open tutorial quest ↗" else "$open open quests ↗", 16f, Palette.LINK, key = "trainer-quests").apply {
                setPadding(0, activity.dp(12), 0, activity.dp(12))
                setOnClickListener { activity.openLink("https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/tutorial-quests.md") }
            })
        }
        body.section("Trainer key", "Your trainer key is your Verse world key: the one over your head in the Grid. Sign a reproduction with it " +
            "on your computer (`microcoder xp reproduce --key`), and the award shows here and in the Grid.") {
            val npub = card.optString("npub")
            add(key(npub, "trainer-npub")); rowDivider(); add(copyRow("Copy npub", npub, "trainer-copy-npub"))
            rowDivider()
            val secret = nsec
            if (secret != null) {
                add(key(secret, "trainer-nsec").apply { setTextIsSelectable(false) }); rowDivider()
                add(copyRow("Copy nsec", secret, "trainer-copy-nsec", secret = true)); rowDivider()
                add(activity.label("Hide nsec", 16f, Palette.LINK).apply {
                    setPadding(0, activity.dp(12), 0, activity.dp(12)); setOnClickListener { hideNsec(); refresh() }
                })
            } else add(activity.label("Reveal nsec", 16f, Palette.FAILURE, key = "trainer-reveal").apply {
                setPadding(0, activity.dp(12), 0, activity.dp(12))
                setOnClickListener {
                    reveal("Reveal your trainer nsec?", "Anyone with this nsec can act as you in Verse and sign work in your name. " +
                        "It can't reach your computers or your wallet. Never share it, and make sure no one can see your screen.",
                        { done -> bridge.trainer(reveal = true, preview = preview) { done(it.textOrNull("nsec")) } }, refresh)
                }
            })
        }
        return ScrollView(activity).apply { addView(body) }
    }

    /** A tappable row in a section card. */
    private fun action(title: String, key: String?, color: Int = Palette.LINK, enabled: Boolean = true, onClick: () -> Unit): TextView =
        activity.label(title, 16f, color, key = key).apply {
            setPadding(0, activity.dp(12), 0, activity.dp(12))
            enabled(enabled)
            setOnClickListener { if (isEnabled) onClick() }
        }

    private fun note(text: String, color: Int = Palette.SECONDARY, mono: Boolean = false, selectable: Boolean = false): TextView =
        activity.label(text, 13f, color, mono = mono, selectable = selectable).apply { setPadding(0, activity.dp(8), 0, activity.dp(8)) }

    /** Asks first, then runs `confirmed`: every trainer publish waits for this. */
    private fun confirm(title: String, message: String, button: String, confirmed: () -> Unit) {
        dialog().setTitle(title).setMessage(message)
            .setPositiveButton(button) { _, _ -> confirmed() }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** Show my level / Hide my level: publishing the trainer profile, after a confirmation to show it. */
    private fun profileSection(body: LinearLayout, card: JSONObject, refresh: () -> Unit) {
        val publishing = card.optString("profile_status") == "publishing"
        val update: (JSONObject) -> Unit = { next -> this.card = next; refresh() }
        body.section("Level over your head", "Other players see your level only after you choose to show it. " +
            "Your XP stays public either way.") {
            if (card.optString("profile") == "shown") {
                add(note("Shown in the Grid and on boards", Palette.PRIMARY)); rowDivider()
                add(action("Hide my level", "trainer-hide-level", enabled = !publishing) { bridge.trainerProfile(false, update) })
            } else {
                add(note(if (card.optString("profile") == "hidden") "Hidden" else "Not shown yet")); rowDivider()
                add(action("Show my level", "trainer-show-level", enabled = !publishing) {
                    confirm("Show your level?", "This publishes a trainer profile signed by your trainer key to relay.openagents.com. " +
                        "Your level then shows over your head in the Grid and on boards. You can hide it again at any time.", "Show") {
                        bridge.trainerProfile(true, update)
                    }
                })
            }
            if (publishing) { rowDivider(); add(note("Publishing…")) }
            card.textOrNull("profile_error")?.let { rowDivider(); add(note(it, Palette.FAILURE)) }
        }
    }

    /** Linked keys: each key the profile lists, linked both ways or waiting, with Remove, and Link a key. */
    private fun linkedKeysSection(body: LinearLayout, card: JSONObject, refresh: () -> Unit) {
        val publishing = card.optString("profile_status") == "publishing"
        val update: (JSONObject) -> Unit = { next -> this.card = next; refresh() }
        body.section("Linked keys", "Sign work on a computer with its own key and have it count here, without moving your trainer key. " +
            "Enter that key's npub, then on the computer run: microcoder xp link --relay ${card.optString("relay")} --trainer ${card.optString("npub")}") {
            card.textOrNull("linked_to")?.let {
                add(note("This key is linked to the trainer ${it.take(16)}…, so its XP counts there.")); rowDivider()
            }
            for (linked in card.optJSONArray("linked_keys")?.objects() ?: emptyList()) {
                val isLinked = linked.optString("status") == "linked"
                add(activity.row().apply {
                    gravity = Gravity.CENTER_VERTICAL
                    setPadding(0, activity.dp(8), 0, activity.dp(8))
                    tag = "trainer-linked-${linked.optString("public_hex").take(16)}"
                    addView(activity.column().apply {
                        add(activity.label(linked.optString("npub").take(20) + "…", 14f, mono = true))
                        add(activity.label(if (isLinked) "Linked both ways: its XP counts here" else "Waiting for this key to link back",
                            12f, if (isLinked) Palette.SUCCESS else Palette.SECONDARY), 2)
                    }, LinearLayout.LayoutParams(0, -2, 1f))
                    addView(action("Remove", null, Palette.FAILURE, enabled = !publishing) {
                        bridge.trainerLink(remove = linked.optString("public_hex"), received = update)
                    })
                })
                rowDivider()
            }
            add(action("Link a key", "trainer-link-key", enabled = !publishing) { askLinkKey(update) })
        }
    }

    private fun askLinkKey(update: (JSONObject) -> Unit) {
        val field = android.widget.EditText(activity).apply {
            hint = "npub1…"; tag = "trainer-link-field"; isSingleLine = true
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
        }
        val frame = android.widget.FrameLayout(activity).apply {
            setPadding(activity.dp(20), activity.dp(8), activity.dp(20), 0); addView(field)
        }
        dialog().setTitle("Link a key")
            .setMessage("This publishes your trainer profile listing the key. Its XP counts here only after that key signs a link back to you.")
            .setView(frame)
            .setPositiveButton("Link") { _, _ -> bridge.trainerLink(add = field.text.toString().trim(), received = update) }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** The last export's reply: the signed card's JSON, file name, and link, or an error. */
    private var export: JSONObject? = null
    /** The card JSON waiting for the file the person picks; kept only until it is written. */
    private var pendingCard: String? = null
    private var saveRefresh: (() -> Unit)? = null
    private val cardSaver = activity.registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.CreateDocument("application/json")) { uri ->
        val text = pendingCard; pendingCard = null
        if (uri == null || text == null) return@registerForActivityResult
        try {
            activity.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) }
                ?: throw IllegalStateException("The file couldn't be opened.")
        } catch (problem: Exception) {
            export = export?.put("error", problem.message ?: "The card couldn't be saved.")
            saveRefresh?.invoke()
        }
    }

    /** Export card: signs and publishes the card after a confirmation; then Share link and Save card JSON. */
    private fun exportSection(body: LinearLayout, card: JSONObject, refresh: () -> Unit) {
        body.section("Trainer card", "A signed summary of your level, keys, and counted awards. " +
            "Anyone can check it with openagents xp verify-card.") {
            add(action("Export card", "trainer-export") {
                confirm("Export your trainer card?", "This signs your card with your trainer key and publishes it to relay.openagents.com, " +
                    "so its link opens a public page. It lists your level, your linked keys, and your counted awards.", "Export") {
                    bridge.trainerExport { reply -> export = reply; refresh() }
                }
            })
            val export = export ?: return@section
            export.textOrNull("error")?.let { rowDivider(); add(note(it, Palette.FAILURE)) }
            export.textOrNull("link")?.let { link ->
                rowDivider()
                add(action("Share link", "trainer-share-link") {
                    activity.startActivity(android.content.Intent.createChooser(android.content.Intent(android.content.Intent.ACTION_SEND)
                        .setType("text/plain").putExtra(android.content.Intent.EXTRA_TEXT, link), "Share"))
                })
                add(note(link, mono = true, selectable = true).apply { tag = "trainer-card-link" })
            }
            val cardJson = export.textOrNull("json")
            val name = export.textOrNull("file_name")
            if (cardJson != null && name != null) {
                rowDivider()
                add(action("Save card JSON", "trainer-save-card") {
                    pendingCard = cardJson; saveRefresh = refresh
                    cardSaver.launch(name)
                })
            }
            val status = card.optString("card_status")
            rowDivider()
            add(note(when {
                export.optBoolean("preview") -> "Preview: signed, not published."
                status == "published" -> "Published to ${card.optString("relay")}."
                status == "failed" -> "The relay didn't take the card. Export again."
                else -> "Publishing…"
            }, if (status == "failed" && !export.optBoolean("preview")) Palette.FAILURE else Palette.SECONDARY).apply { tag = "trainer-card-status" })
        }
    }

    // Your keys (BYOK, #10176)

    /**
     * The person's own OpenRouter, Vercel AI Gateway, and TypeSafe keys. Rust
     * tests each key and writes every row; this screen collects a key in a
     * masked field and never draws one, only its last four characters.
     */
    fun yourKeys(state: JSONObject?, refresh: () -> Unit): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        if (state == null) {
            body.add(note("Your keys load in a moment."))
            return ScrollView(activity).apply { addView(body) }
        }
        val mine = state.optBoolean("mine")
        val blocked = state.textOrNull("mine_blocked")
        body.section(null, if (!mine && blocked != null) blocked
            else "Chat replies, Jev, and search run on your own provider accounts. Nothing falls back to OpenAgents.") {
            add(note(state.optString("status"), Palette.PRIMARY).apply { tag = "keys-status" }); rowDivider()
            add(action(if (mine) "Use my keys for everything: on" else "Use my keys for everything: off", "keys-mine",
                enabled = mine || blocked == null) { bridge.providerKeysMine(!mine) })
        }
        for (row in state.optJSONArray("rows")?.objects() ?: emptyList()) {
            val provider = row.optString("provider")
            val last = row.textOrNull("last_four")
            val checking = row.optBoolean("checking")
            body.section(row.optString("name")) {
                val word = if (checking) "checking…" else row.textOrNull("state")
                add(activity.label(listOfNotNull(last?.let { "Ends in $it" } ?: "Not added", word).joinToString(" · "), 16f,
                    if (last == null) Palette.SECONDARY else Palette.PRIMARY, key = "keys-$provider-row").apply {
                    setPadding(0, activity.dp(12), 0, activity.dp(12))
                })
                row.textOrNull("line")?.let { rowDivider(); add(note(it)) }
                rowDivider()
                add(action(if (last == null) "Add key" else "Replace key", "keys-$provider-add") { askKey(row) })
                if (last != null) {
                    rowDivider()
                    add(action("Test", "keys-$provider-test", enabled = !checking) { bridge.providerKeyTest(provider) })
                    rowDivider()
                    add(action("Remove", "keys-$provider-remove", Palette.FAILURE) {
                        confirm("Remove your ${row.optString("name")} key?", "It is deleted from this phone.", "Remove") {
                            bridge.providerKeyRemove(provider)
                        }
                    })
                }
                rowDivider()
                add(action("Make a key at ${row.optString("name")}", null) { activity.openLink(row.optString("page")) })
            }
        }
        (bridge.providerKeyError ?: state.textOrNull("notice"))?.let { body.section(null) { add(note(it).apply { tag = "keys-notice" }) } }
        return ScrollView(activity).apply { addView(body) }
    }

    /** "Use your keys for everything?", once, after a key that can answer chat was added. */
    fun askMine() = confirm("Use your keys for everything?",
        "Chat replies, Jev, and search will run on your own provider accounts, never on OpenAgents.", "Use my keys") {
        bridge.providerKeysMine(true)
    }

    /**
     * The masked field for one provider's key (`provider_key`): no
     * suggestions, no autofill, no screenshots while it shows, and the value
     * goes only to Rust's test.
     */
    private fun askKey(row: JSONObject) {
        val input = row.objectOrNull("input")
        val max = input?.optInt("max_bytes", 512) ?: 512
        val field = android.widget.EditText(activity).apply {
            hint = input?.optString("label") ?: "Key"; tag = "keys-entry"; isSingleLine = true
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD or
                android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
            filters = arrayOf(android.text.InputFilter.LengthFilter(max))
        }
        val frame = android.widget.FrameLayout(activity).apply {
            setPadding(activity.dp(20), activity.dp(8), activity.dp(20), 0); addView(field)
        }
        val shown = dialog().setTitle(row.optString("name"))
            .setMessage(input?.optString("prompt") ?: "")
            .setView(frame)
            .setPositiveButton("Add") { _, _ ->
                bridge.providerKeyAdd(row.optString("provider"), field.text.toString())
                field.text.clear()
            }
            .setNegativeButton("Cancel") { _, _ -> field.text.clear() }
            .create()
        shown.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        shown.show()
    }

    // About this device

    fun about(packet: JSONObject?): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        body.section("Device public key", account?.textOrNull("origin")) {
            add(key(packet?.textOrNull("device_npub"), "device-npub")); rowDivider(); add(key(packet?.textOrNull("device"), "device-key"))
        }
        body.section("App version") {
            add(activity.label("${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})", 16f, selectable = true).apply {
                setPadding(0, activity.dp(12), 0, activity.dp(12))
            })
        }
        return ScrollView(activity).apply { addView(body) }
    }

    // Changelog

    fun changelog(): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        for (release in account?.optJSONArray("changelog")?.objects() ?: emptyList()) {
            val heading = activity.row().apply {
                gravity = Gravity.BOTTOM; setPadding(activity.dp(16), activity.dp(20), 0, 0)
                val build = release.optString("build")
                val version = release.getString("version")
                addView(activity.label(if (build.isEmpty()) version else "$version ($build)", 17f, bold = true))
                addView(activity.label(release.getString("title"), 14f, Palette.SECONDARY),
                    LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(10) })
            }
            body.add(heading)
            val card = activity.column().apply {
                background = activity.rounded(Palette.SURFACE, 12f)
                setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(4))
                val whatToTest = release.optString("what_to_test")
                if (whatToTest.isNotEmpty()) {
                    add(activity.column().apply {
                        setPadding(0, activity.dp(10), 0, activity.dp(10))
                        add(activity.label("What to test", 16f, bold = true))
                        add(activity.label(whatToTest, 14f), 2)
                    })
                }
                release.getJSONArray("items").objects().forEachIndexed { index, item ->
                    if (index > 0 || whatToTest.isNotEmpty()) rowDivider()
                    add(activity.column().apply {
                        setPadding(0, activity.dp(10), 0, activity.dp(10))
                        add(activity.label(item.getString("title"), 16f, bold = true))
                        add(activity.label(item.getString("detail"), 14f, Palette.SECONDARY), 2)
                    })
                }
            }
            body.add(card, 6)
        }
        return ScrollView(activity).apply { addView(body) }
    }

    // Computers

    /**
     * One row per computer: a status dot, its name, and a short status. A
     * tap opens it; a long press offers its menu, and a destructive choice
     * asks first.
     */
    fun computers(home: JSONObject): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        val empty = home.textOrNull("empty")
        if (empty != null) {
            body.section(null) {
                add(activity.column().apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                    setPadding(0, activity.dp(24), 0, activity.dp(24))
                    add(android.widget.ImageView(activity).apply {
                        setImageResource(R.drawable.ic_computer); importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
                    }, 0, -2)
                    add(activity.label(empty, 14f, Palette.SECONDARY).apply { gravity = Gravity.CENTER }, 12)
                    add(activity.pill(home.getString("connect"), "computers-connect", primary = true) { bridge.connectOpen() }, 16, -2)
                })
            }
        } else {
            body.section(null) {
                add(activity.pill(home.getString("connect"), "computers-connect", primary = true) { bridge.connectOpen() }, 0, -2)
            }
            body.section(null, home.textOrNull("notice")) {
                home.getJSONArray("rows").objects().forEachIndexed { index, row ->
                    if (index > 0) rowDivider()
                    add(computerRow(row))
                }
            }
        }
        return ScrollView(activity).apply { addView(body) }
    }

    private fun computerRow(row: JSONObject): View = activity.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = activity.dp(56)
        setPadding(0, activity.dp(8), 0, activity.dp(8))
        val name = row.getString("name")
        tag = "computer-$name"
        val tone = when (row.optString("tone")) {
            "online" -> Palette.SUCCESS; "pending" -> 0xFFFFD60A.toInt(); "alert" -> Palette.FAILURE; else -> 0xFF737373.toInt()
        }
        addView(View(activity).apply { background = activity.rounded(tone, 4f); importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO },
            LinearLayout.LayoutParams(activity.dp(8), activity.dp(8)))
        addView(activity.column().apply {
            add(activity.label(name, 16f))
            add(activity.label(row.getString("status"), 14f, Palette.SECONDARY), 2)
            row.textOrNull("watchers")?.let { add(activity.label(it, 13f, Palette.SECONDARY), 2) }
            row.textOrNull("background")?.let { add(activity.label(it, 13f, Palette.SECONDARY), 2) }
        }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = activity.dp(12) })
        addView(activity.text("›", 22f, Palette.TERTIARY))
        contentDescription = listOfNotNull(name, row.getString("status"), row.textOrNull("watchers"), row.textOrNull("background")).joinToString(", ")
        val host = row.getString("host")
        val menu = row.optJSONArray("menu")?.objects() ?: emptyList()
        setOnClickListener { bridge.openComputer(host) }
        setOnLongClickListener { view ->
            if (menu.isEmpty()) return@setOnLongClickListener false
            val popup = PopupMenu(activity, view, Gravity.END)
            menu.forEachIndexed { i, item -> popup.menu.add(0, i, i, item.getString("label")) }
            popup.setOnMenuItemClickListener { choice -> menu.getOrNull(choice.itemId)?.let { choose(host, it) }; true }
            popup.show(); true
        }
        // TalkBack reaches the row's menu as custom actions.
        menu.forEach { item ->
            androidx.core.view.ViewCompat.addAccessibilityAction(this, item.getString("label")) { _, _ -> choose(host, item); true }
        }
    }

    private fun choose(host: String, item: JSONObject) {
        val confirm = item.textOrNull("confirm")
        if (confirm == null) { bridge.chooseComputer(host, item.getString("choice")); return }
        dialog().setTitle(confirm)
            .setPositiveButton(item.getString("label")) { _, _ -> bridge.chooseComputer(host, item.getString("choice")) }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** The list's own menu: Activity, Refresh, and the owner-directory controls when they apply. */
    fun listMenu(anchor: View, home: JSONObject) {
        val entries = buildList {
            add(home.getString("add_other") to "add"); add("Activity" to "activity"); add("Refresh" to "refresh")
            if (home.optBoolean("owner_key")) add("Enter owner key" to "owner_key")
            if (home.optBoolean("keep_directory")) add("Keep this device's version" to "keep_directory")
        }
        val popup = PopupMenu(activity, anchor, Gravity.END)
        entries.forEachIndexed { i, (label, _) -> popup.menu.add(0, i, i, label) }
        popup.setOnMenuItemClickListener { item -> bridge.computersGo(entries[item.itemId].second); true }
        popup.show()
    }

    internal fun TextView.boldText(): TextView { typeface = PaperMono.typeface(context, PaperMono.BOLD); return this }
}
