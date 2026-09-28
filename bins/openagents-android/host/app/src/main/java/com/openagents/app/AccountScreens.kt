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

    private fun warn(refresh: () -> Unit) {
        dialog().setTitle("Reveal your nsec?")
            .setMessage("Anyone with your nsec can act as this device on your computers. Never share it, and make sure no one can see your screen.")
            .setPositiveButton("Reveal") { _, _ ->
                bridge.account(reveal = true) { packet ->
                    nsec = packet.textOrNull("nsec") ?: return@account
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
