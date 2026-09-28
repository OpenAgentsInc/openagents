// The Account tab's own screens: identity keys, this device, the changelog,
// and the native Computers list. Rust derives every key form, owns the
// changelog, and builds every Computers row and choice; these views only
// show, copy, and forward what it returns. They follow the iOS
// `AccountScreens.swift` and the Computers list in `OpenAgentsApp.swift`.
package com.openagents.app

import android.app.AlertDialog
import android.graphics.Typeface
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

    /** Reads the keys and changelog (never the nsec) for the Account screens. */
    fun load(refresh: () -> Unit) = bridge.account { account = it; refresh() }

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
            add(activity.label("Over your head in the Grid: ${card.optString("tag")}${if (xp > 0) " · lv $level" else ""}", 13f,
                Palette.SECONDARY, mono = true).apply { setPadding(0, 0, 0, activity.dp(10)) }, 2)
        }
        card ?: return ScrollView(activity).apply { addView(body) }
        val titles = card.optJSONArray("titles").strings()
        if (titles.isNotEmpty()) body.section("Titles") {
            add(activity.label(titles.joinToString(", "), 16f).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) })
        }
        body.section("Counted awards", "Trusting the OpenAgents referee, ${card.optString("referee_npub").take(16)}…, on " +
            "${card.optString("relay")}. ${card.optString("note")}") {
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
                addView(activity.label(release.getString("version"), 17f, bold = true))
                addView(activity.label(release.getString("title"), 14f, Palette.SECONDARY),
                    LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(10) })
            }
            body.add(heading)
            val card = activity.column().apply {
                background = activity.rounded(Palette.SURFACE, 12f)
                setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(4))
                release.getJSONArray("items").objects().forEachIndexed { index, item ->
                    if (index > 0) rowDivider()
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
                    add(activity.pill("Add a computer", "computers-add-empty", primary = true) { bridge.computersGo("add") }, 16, -2)
                })
            }
        } else {
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
        }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = activity.dp(12) })
        addView(activity.text("›", 22f, Palette.TERTIARY))
        contentDescription = "$name, ${row.getString("status")}"
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
            add("Activity" to "activity"); add("Refresh" to "refresh")
            if (home.optBoolean("owner_key")) add("Enter owner key" to "owner_key")
            if (home.optBoolean("keep_directory")) add("Keep this device's version" to "keep_directory")
        }
        val popup = PopupMenu(activity, anchor, Gravity.END)
        entries.forEachIndexed { i, (label, _) -> popup.menu.add(0, i, i, label) }
        popup.setOnMenuItemClickListener { item -> bridge.computersGo(entries[item.itemId].second); true }
        popup.show()
    }

    internal fun TextView.boldText(): TextView { setTypeface(typeface, Typeface.BOLD); return this }
}
