// The Wallet tab: bitcoin on mainnet through Breez's Spark SDK. Rust runs
// the wallet and decides every state and line of text; this screen lays them
// out, collects typed values, and adds the clipboard, the camera, and the
// browser. The seed is kept encrypted under its own Android Keystore key
// (DeviceKey.Purpose.SPARK); the recovery words reach this screen only in a
// direct reply, and only while their dialog is open. It follows the iOS
// Wallet tab (`WalletTab.swift`).
package com.openagents.app

import android.app.AlertDialog
import android.content.ClipboardManager
import android.content.Intent
import android.graphics.drawable.BitmapDrawable
import android.text.InputType
import android.text.format.DateUtils
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject

class WalletScreen(private val activity: MainActivity, private val bridge: MobileBridge, private val scanner: QRScanner) {
    /** What the main screen shows under the two buttons. */
    private enum class Mode { HOME, RECEIVE, SEND }
    /** The other ways to receive, under Advanced. */
    private enum class Method(val title: String, val key: String) { LIGHTNING("Lightning", "lightning"), SPARK("Spark", "spark"), BITCOIN("Bitcoin", "bitcoin"), NOSTR("Nostr", "nostr") }

    val root = FrameLayout(activity)
    private val scroll = ScrollView(activity)
    private val content = activity.column()
    private val infoButton = activity.text("ⓘ", 24f).apply {
        gravity = Gravity.CENTER; contentDescription = "About this wallet"; tag = "wallet-info"
        setOnClickListener { showTrust() }
    }
    private var mode = Mode.HOME
    private var method = Method.SPARK
    private var scanning = false
    private var shown: String? = null
    private var copied: String? = null

    // Fields keep what the person typed across Rust's updates.
    private val invoiceAmount = field("Amount in ₿ (optional)", "wallet-invoice-amount", number = true)
    private val payInput = field("Paste or scan", "wallet-send-input", lines = 4)
    private val payComment = field("Note (optional)", "wallet-send-comment")
    private val contactName = field("Name", "wallet-contact-name")
    private val refundAddress = field("Bitcoin address to refund to", "wallet-refund-address")
    private var refundSpeed = "medium"
    private var exportError: String? = null
    /** The exit backup waiting for the file the person picks; kept only until it is written. */
    private var pendingExport: String? = null
    private val exporter = activity.registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.CreateDocument("application/json")) { uri ->
        val text = pendingExport; pendingExport = null
        if (uri == null || text == null) return@registerForActivityResult
        exportError = try {
            activity.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) }
                ?: throw IllegalStateException("The file couldn't be opened.")
            null
        } catch (problem: Exception) { problem.message ?: "The backup couldn't be saved." }
        redraw()
    }
    private val payAmount = field("Amount in ₿", "wallet-send-amount", number = true)
    private val buyAmount = field("Amount in ₿", "wallet-buy-amount", number = true)
    // Rust's amount format (`amounts::AmountsView`): BIP 177 or legacy BTC.
    private var amounts: JSONObject? = null
    private val camera = FrameLayout(activity)
    private var review: View? = null
    private var reviewReady: () -> Boolean = { false }

    init {
        val page = activity.column()
        val header = activity.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(activity.dp(20), activity.dp(12), activity.dp(12), 0)
            addView(activity.text("Wallet", 32f).bold(), LinearLayout.LayoutParams(0, -2, 1f))
            addView(infoButton, LinearLayout.LayoutParams(activity.dp(48), activity.dp(48)))
        }
        page.addView(header)
        content.setPadding(activity.dp(20), activity.dp(8), activity.dp(20), activity.dp(32))
        scroll.addView(content)
        page.addView(scroll, LinearLayout.LayoutParams(-1, 0, 1f))
        root.addView(page, FrameLayout.LayoutParams(-1, -1))
        payInput.addTextChangedListener(object : android.text.TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
            override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
            override fun afterTextChanged(s: android.text.Editable?) { review?.enabled(reviewReady()) }
        })
    }

    private fun field(hint: String, key: String, number: Boolean = false, lines: Int = 1) = EditText(activity).apply {
        this.hint = hint; tag = key; contentDescription = hint
        textSize = 15f; setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
        background = activity.rounded(Palette.RAISED, 10f)
        setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
        isSaveEnabled = false
        inputType = if (number) InputType.TYPE_CLASS_NUMBER
            else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING or EditorInfo.IME_ACTION_DONE
        maxLines = lines
        importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
    }

    /** The tab is showing: hand Rust the seed once. */
    fun appeared() = bridge.openWallet()

    /** Redraws when Rust's Wallet state changed. */
    fun update(packet: JSONObject?, force: Boolean = false) {
        val wallet = packet?.objectOrNull("wallet")
        amounts = packet?.objectOrNull("amounts")
        val key = "${wallet?.toString()}|${amounts?.toString()}|$mode|$method|$scanning|$copied"
        if (!force && key == shown) return
        shown = key
        applyFormat()
        val focused = listOf(invoiceAmount, payInput, payAmount, buyAmount, payComment, contactName, refundAddress).firstOrNull { it.hasFocus() }
        val y = scroll.scrollY
        content.removeAllViews()
        infoButton.enabled(wallet?.objectOrNull("trust") != null)
        if (wallet?.optString("state") == "failed") failed(wallet) else ready(wallet ?: opening())
        focused?.requestFocus()
        scroll.post { scroll.scrollTo(0, y) }
    }

    private fun redraw() = update(bridge.packet, force = true)

    /** Amount fields name the unit and take a decimal point in legacy BTC. */
    private fun applyFormat() {
        val unit = amounts?.textOrNull("unit") ?: "₿"
        val decimal = amounts?.optBoolean("decimal") == true
        listOf(invoiceAmount to "Amount in $unit (optional)", payAmount to "Amount in $unit",
            buyAmount to "Amount in $unit").forEach { (field, hint) ->
            field.hint = hint; field.contentDescription = hint
            field.inputType = if (decimal) InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL else InputType.TYPE_CLASS_NUMBER
        }
    }

    private fun amountSetting() {
        content.add(activity.label("Show amounts as", 17f, bold = true), 24)
        content.add(formatPicker(), 10)
        content.add(activity.label("Applies everywhere in the app. Stored amounts don't change.", 13f, Palette.SECONDARY), 6)
    }

    private fun formatPicker(): View {
        val choices = amounts?.optJSONArray("choices")?.objects() ?: emptyList()
        val selected = choices.indexOfFirst { it.optBoolean("selected") }.coerceAtLeast(0)
        return segments(choices.map { it.optString("label") }, selected, "amount-format") { index ->
            // What was typed was in the old format; start over.
            listOf(invoiceAmount, payAmount, buyAmount).forEach { it.setText("") }
            bridge.wallet("amount_format", "format" to choices[index].optString("id"))
        }
    }

    private fun opening() = json("state" to "ready", "network" to "Bitcoin · Spark", "refreshing" to true,
        "balance_unknown" to true, "status" to "Opening the wallet…", "can_show_words" to false)

    private fun failed(wallet: JSONObject) {
        content.add(activity.label(wallet.optString("message", "The wallet could not start."), 16f), 16)
        content.add(activity.pill("Try again", "wallet-retry") { bridge.wallet("wallet_refresh") }, 16, -2)
        content.add(activity.pill("Restore from recovery words", "wallet-restore") { restore(false) }, 10, -2)
    }

    private fun ready(wallet: JSONObject) {
        balance(wallet)
        wallet.textOrNull("warning")?.let { warning ->
            content.add(activity.label(warning, 13f, key = "wallet-warning").apply {
                setPadding(activity.dp(12), activity.dp(12), activity.dp(12), activity.dp(12))
                background = activity.rounded(0xFF1F1F1F.toInt(), 12f)
            }, 16)
        }
        wallet.objectOrNull("backup_card")?.let { backupCard(it, wallet) }
        primaryButtons(wallet)
        when (activeMode(wallet)) {
            Mode.RECEIVE -> receive(wallet)
            Mode.SEND -> send(wallet)
            Mode.HOME -> Unit
        }
        recent(wallet)
        advanced(wallet)
    }

    /** A payment in progress keeps the Send panel open. */
    private fun activeMode(wallet: JSONObject): Mode {
        val state = wallet.objectOrNull("send")?.optString("state") ?: "idle"
        return if (state != "idle" && (state != "failed" || mode == Mode.SEND)) Mode.SEND else mode
    }

    /** Shown until the person writes down this wallet's recovery words. */
    private fun backupCard(card: JSONObject, wallet: JSONObject) {
        val box = activity.column().apply {
            setPadding(activity.dp(14), activity.dp(14), activity.dp(14), activity.dp(14))
            background = activity.rounded(0xFF1F1F1F.toInt(), 14f)
            add(activity.label(card.getString("title"), 17f, bold = true))
            add(activity.label(card.getString("detail"), 13f, Palette.SECONDARY), 4)
            add(activity.pill(card.getString("action"), "wallet-backup-card") { confirmWords() }
                .enabled(wallet.optBoolean("can_show_words")), 10, -2)
        }
        content.add(box, 16)
    }

    /** Receive and Send: the screen's two big buttons. */
    private fun primaryButtons(wallet: JSONObject) {
        val active = activeMode(wallet)
        val row = activity.row()
        // White, or dimmed while the other button's panel is open.
        fun big(title: String, key: String, on: Boolean, action: () -> Unit) = activity.text(title, 17f,
            if (!on && active != Mode.HOME) Palette.PRIMARY else Palette.BACKGROUND).apply {
            gravity = Gravity.CENTER; typeface = Fonts.typeface(context, Fonts.BOLD)
            setPadding(0, activity.dp(16), 0, activity.dp(16))
            background = activity.rounded(if (!on && active != Mode.HOME) 0xFF333333.toInt() else Palette.PRIMARY, 14f)
            tag = key; contentDescription = title; isSelected = on
            setOnClickListener { action() }
        }
        row.addView(big("↓  Receive", "wallet-receive", active == Mode.RECEIVE) {
            bridge.wallet("wallet_send_reset")
            if (mode == Mode.RECEIVE) { mode = Mode.HOME; redraw(); return@big }
            mode = Mode.RECEIVE
            // One request for any amount, ready to scan.
            val receive = wallet.objectOrNull("receive")
            if (receive?.objectOrNull("lightning") == null && receive?.optBoolean("lightning_busy") != true &&
                wallet.textOrNull("status") == null) bridge.wallet("wallet_invoice", "amount" to "")
            redraw()
        }, LinearLayout.LayoutParams(0, -2, 1f))
        row.addView(big("↑  Send", "wallet-send", active == Mode.SEND) {
            if (mode == Mode.SEND) bridge.wallet("wallet_send_reset")
            mode = if (mode == Mode.SEND) Mode.HOME else Mode.SEND
            redraw()
        }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = activity.dp(12) })
        content.add(row, 20)
    }

    /** The newest payments, with See all for the rest. */
    private fun recent(wallet: JSONObject) {
        val header = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
        header.addView(activity.label("Recent activity", 17f, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
        if (wallet.optBoolean("more_payments")) header.addView(activity.pill("See all", "wallet-see-all") {
            showHistory(wallet.optJSONArray("payments")?.objects() ?: emptyList())
        })
        content.add(header, 24)
        val recent = wallet.optJSONArray("recent")?.objects() ?: emptyList()
        if (recent.isEmpty()) content.add(activity.label("Payments you send and receive appear here.", 13f, Palette.SECONDARY), 6)
        for (payment in recent) content.add(paymentRow(payment, method = false), 10)
    }

    private fun showHistory(payments: List<JSONObject>) {
        val body = activity.column().apply {
            setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(8))
            for (payment in payments) add(paymentRow(payment, method = true), 10)
        }
        dialog().setTitle("Activity").setView(ScrollView(activity).apply { addView(body) })
            .setPositiveButton("Done", null).show()
    }

    /** Everything else, closed by default and remembered on this phone. */
    private fun advanced(wallet: JSONObject) {
        val advanced = wallet.objectOrNull("advanced")
        val open = advanced?.optBoolean("open") == true
        val header = activity.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, activity.dp(12), 0, activity.dp(12))
            val title = activity.column().apply {
                add(activity.label("Advanced", 17f, bold = true))
                if (!open) advanced?.textOrNull("note")?.let { add(activity.label(it, 13f, Palette.SECONDARY), 2) }
            }
            addView(title, LinearLayout.LayoutParams(0, -2, 1f))
            addView(activity.label(if (open) "⌃" else "⌄", 20f))
            tag = "wallet-advanced"; contentDescription = if (open) "Advanced, open" else "Advanced"
            setOnClickListener { bridge.wallet("wallet_advanced", "open" to !open) }
        }
        content.add(header, 24)
        if (!open) return
        details(wallet)
        otherWays(wallet)
        buy(wallet)
        val deposits = wallet.optJSONArray("deposits")?.objects() ?: emptyList()
        if (deposits.isNotEmpty()) deposits(deposits, wallet.objectOrNull("claim"), wallet.objectOrNull("refund"))
        people(wallet)
        bridge.packet?.objectOrNull("spend")?.let { AgentPayments.section(activity, content, it, bridge) }
        if (amounts != null) amountSetting()
        recovery(wallet)
        wallet.objectOrNull("backup")?.let { backup(it) }
    }

    /** The balance in the other unit, the network, and when it was read. */
    private fun details(wallet: JSONObject) {
        content.add(activity.label("Balance", 17f, bold = true), 16)
        wallet.textOrNull("balance_alternate")?.let { content.add(line("Also", it, mono = true), 6) }
        content.add(line("Network", wallet.optString("network", "Bitcoin · Spark")), 6)
        val status = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
        val synced = if (wallet.has("synced_at") && !wallet.isNull("synced_at")) wallet.getLong("synced_at") else null
        status.addView(activity.label(if (wallet.optBoolean("refreshing")) "Refreshing…" else updated(synced), 13f, Palette.SECONDARY),
            LinearLayout.LayoutParams(0, -2, 1f))
        status.addView(activity.pill("Refresh", "wallet-refresh") { bridge.wallet("wallet_refresh") })
        content.add(status, 6)
    }

    /** Contacts and people paid before: tap to pay. */
    private fun people(wallet: JSONObject) {
        val people = wallet.optJSONArray("people")?.objects() ?: emptyList()
        if (people.isEmpty()) return
        content.add(activity.label("People", 17f, bold = true), 24)
        for (person in people) content.add(activity.column().apply {
            setPadding(activity.dp(12), activity.dp(8), activity.dp(12), activity.dp(8))
            background = activity.rounded(0xFF1A1A1A.toInt(), 10f)
            add(activity.label(person.getString("name"), 14f))
            add(activity.label(person.getString("detail"), 11f, Palette.SECONDARY).apply { maxLines = 1 }, 2)
            tag = "wallet-person"; contentDescription = "Pay ${person.getString("name")}, ${person.getString("detail")}"
            setOnClickListener {
                val input = person.getString("input")
                payInput.setText(input); mode = Mode.SEND
                quote(input)
            }
        }, 8)
    }

    private fun balance(wallet: JSONObject) {
        val unknown = wallet.optBoolean("balance_unknown")
        content.add(activity.label(if (unknown) "₿—" else wallet.optString("balance"), 48f, bold = true, key = "wallet-balance").apply {
            maxLines = 1; if (unknown) setTextColor(Palette.TERTIARY)
            contentDescription = if (unknown) "Balance not read yet" else wallet.optString("balance_spoken", wallet.optString("balance"))
        }, 8)
        // Quiet: only while starting, or when the balance is old or failed to update.
        val status = wallet.textOrNull("status")
        if (status != null) {
            val row = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
            row.addView(ProgressBar(activity).apply { isIndeterminate = true }, LinearLayout.LayoutParams(activity.dp(18), activity.dp(18)))
            row.addView(activity.label(status, 13f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
            content.add(row, 6)
        } else wallet.textOrNull("error")?.let { content.add(activity.label(it, 13f, Palette.SECONDARY), 6) }
            ?: if (wallet.optBoolean("stale") && !wallet.optBoolean("refreshing")) {
                val synced = if (wallet.has("synced_at") && !wallet.isNull("synced_at")) wallet.getLong("synced_at") else null
                content.add(activity.label(updated(synced), 13f, Palette.SECONDARY), 6)
            } else Unit
        if (wallet.optBoolean("empty")) content.add(activity.label("No bitcoin yet. Tap Receive to get some.",
            14f, Palette.SECONDARY), 6)
    }

    private fun segments(titles: List<String>, selected: Int, key: String, choose: (Int) -> Unit): View = activity.row().apply {
        background = activity.rounded(Palette.SURFACE, 10f)
        setPadding(activity.dp(2), activity.dp(2), activity.dp(2), activity.dp(2))
        titles.forEachIndexed { index, title ->
            val on = index == selected
            addView(activity.text(title, 14f, if (on) Palette.BACKGROUND else Palette.PRIMARY).apply {
                gravity = Gravity.CENTER; setPadding(0, activity.dp(8), 0, activity.dp(8))
                if (on) { background = activity.rounded(Palette.PRIMARY, 8f); typeface = Fonts.typeface(context, Fonts.BOLD) }
                tag = "$key-${title.lowercase()}"; isSelected = on
                contentDescription = "$title${if (on) ", selected" else ""}"
                setOnClickListener { if (!on) choose(index) }
            }, LinearLayout.LayoutParams(0, -2, 1f))
        }
    }

    private fun place(view: View, top: Int = 12) {
        (view.parent as? android.view.ViewGroup)?.removeView(view)
        content.addView(view, LinearLayout.LayoutParams(-1, -2).apply { topMargin = activity.dp(top) })
    }

    // Receive

    /** One request that just works: a QR code, Copy, and Share, with an optional amount. */
    private fun receive(wallet: JSONObject) {
        val receive = wallet.objectOrNull("receive")
        val starting = wallet.textOrNull("status") != null
        val busy = receive?.optBoolean("lightning_busy") == true
        receive?.objectOrNull("lightning")?.let { code(it) } ?: if (busy || starting) placeholder() else Unit
        receive?.textOrNull("lightning_error")?.let { content.add(activity.label(it, 13f), 6) }
        val row = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
        (invoiceAmount.parent as? android.view.ViewGroup)?.removeView(invoiceAmount)
        row.addView(invoiceAmount, LinearLayout.LayoutParams(0, -2, 1f))
        row.addView(activity.pill(if (busy) "Making…" else if (receive?.objectOrNull("lightning") == null) "Create" else "Update", "wallet-new-invoice") {
            hideKeyboard(); bridge.wallet("wallet_invoice", "amount" to invoiceAmount.text.toString())
        }.enabled(!busy && !starting), LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
        content.add(row, 12)
        content.add(activity.label("Other ways to receive are under Advanced.", 13f, Palette.SECONDARY), 8)
    }

    /** Lightning, Spark, on-chain Bitcoin, and Nostr, under Advanced. */
    private fun otherWays(wallet: JSONObject) {
        content.add(activity.label("Other ways to receive", 17f, bold = true), 24)
        content.add(segments(Method.entries.map { it.title }, method.ordinal, "wallet-method") { method = Method.entries[it]; redraw() }, 12)
        val receive = wallet.objectOrNull("receive")
        val starting = wallet.textOrNull("status") != null
        when (method) {
            Method.LIGHTNING -> receive?.objectOrNull("lightning")?.let { code(it) }
                ?: content.add(activity.label("Tap Receive above to make a Lightning request.", 13f, Palette.SECONDARY), 8)
            Method.SPARK -> receive?.objectOrNull("spark")?.let { code(it) } ?: placeholder()
            Method.BITCOIN -> receive?.objectOrNull("bitcoin")?.let { code(it) } ?: placeholder()
            Method.NOSTR -> {
                receive?.objectOrNull("nostr")?.let { code(it) } ?: placeholder()
                receive?.objectOrNull("publish")?.let { publish ->
                    val on = publish.optBoolean("on"); val busy = publish.optBoolean("busy")
                    content.add(android.widget.Switch(activity).apply {
                        text = "Publish my Spark address"; setTextColor(Palette.PRIMARY); textSize = 15f
                        isChecked = on; tag = "wallet-publish"
                        isEnabled = !busy && starting.not()
                        setOnCheckedChangeListener { _, value -> if (value != on) bridge.wallet("wallet_publish", "on" to value) }
                    }, 14)
                    content.add(activity.label(publish.optString("detail"), 13f, Palette.SECONDARY), 4)
                    if (busy) content.add(ProgressBar(activity), 6, -2)
                    publish.textOrNull("message")?.let { content.add(activity.label(it, 13f), 6) }
                }
            }
        }
    }

    private fun code(code: JSONObject) {
        code.objectOrNull("qr")?.let { qr ->
            content.add(ImageView(activity).apply {
                setImageDrawable(qrBitmap(qr)?.let { BitmapDrawable(activity.resources, it).apply { isFilterBitmap = false } })
                contentDescription = "QR code"; scaleType = ImageView.ScaleType.FIT_CENTER
            }.also { it.layoutParams = LinearLayout.LayoutParams(activity.dp(220), activity.dp(220)) }, 14)
            (content.getChildAt(content.childCount - 1).layoutParams as LinearLayout.LayoutParams).apply {
                width = activity.dp(220); height = activity.dp(220); gravity = Gravity.CENTER_HORIZONTAL
            }
        }
        content.add(activity.label(code.getString("caption"), 13f, Palette.SECONDARY), 10)
        val text = code.getString("text")
        content.add(activity.label(text, 13f, mono = true, key = "wallet-receive-code", selectable = true).apply {
            maxLines = 3; ellipsize = android.text.TextUtils.TruncateAt.MIDDLE
        }, 6)
        val actions = activity.row()
        actions.addView(activity.pill(if (copied == text) "Copied" else "Copy", "wallet-copy") {
            Clipboard.copy(activity, "Wallet address", text); copied = text; redraw()
            root.postDelayed({ if (copied == text) { copied = null; redraw() } }, 2000)
        })
        actions.addView(activity.pill("Share", "wallet-share") {
            activity.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain")
                .putExtra(Intent.EXTRA_TEXT, text), "Share"))
        }, LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(12) })
        content.add(actions, 10)
    }

    private fun placeholder() {
        content.add(View(activity).apply {
            background = activity.rounded(0x14FFFFFF, 12f); importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }, 14)
        (content.getChildAt(content.childCount - 1).layoutParams as LinearLayout.LayoutParams).apply {
            width = activity.dp(220); height = activity.dp(220); gravity = Gravity.CENTER_HORIZONTAL
        }
        content.add(activity.label("Waiting for the wallet…", 13f, Palette.TERTIARY), 10)
    }

    // Send

    private fun send(wallet: JSONObject) {
        val send = wallet.objectOrNull("send")
        val state = send?.optString("state") ?: "idle"
        when (state) {
            "quoted", "paying" -> send?.objectOrNull("quote")?.let { confirm(it, state == "paying", send.textOrNull("message")) }
            "sent" -> {
                content.add(activity.label(send?.textOrNull("message") ?: "Sent.", 17f, bold = true), 12)
                send?.objectOrNull("result")?.let { content.add(paymentRow(it, method = false), 8) }
                send?.textOrNull("recipient_message")?.let { content.add(activity.label(it, 13f, key = "wallet-recipient-message", selectable = true), 8) }
                send?.textOrNull("save_suggestion")?.let { address ->
                    content.add(activity.label("Save $address for next time?", 13f, Palette.SECONDARY), 12)
                    val row = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
                    (contactName.parent as? android.view.ViewGroup)?.removeView(contactName)
                    row.addView(contactName, LinearLayout.LayoutParams(0, -2, 1f))
                    row.addView(activity.pill("Save", "wallet-contact-save") {
                        if (contactName.text.isNotBlank()) {
                            bridge.wallet("wallet_save_contact", "name" to contactName.text.toString(), "address" to address)
                            contactName.setText("")
                        }
                    }, LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
                    content.add(row, 6)
                }
                content.add(activity.pill("Done", "wallet-send-done", primary = true) {
                    payInput.setText(""); payAmount.setText(""); payComment.setText(""); mode = Mode.HOME
                    bridge.wallet("wallet_send_reset")
                }, 12, -2)
            }
            else -> {
                if (scanning) {
                    content.add(activity.label("Point the camera at a payment QR code.", 13f, Palette.SECONDARY), 12)
                    place(camera, 8)
                    content.add(activity.pill("Type instead", "wallet-type") { stopScan() }, 8, -2)
                    return
                }
                place(payInput)
                val actions = activity.row()
                actions.addView(activity.pill("Paste", "wallet-paste") {
                    val clipboard = activity.getSystemService(ClipboardManager::class.java)
                    clipboard?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(activity)?.let {
                        payInput.setText(it); if (it.isNotBlank()) quote(it.toString())
                    }
                })
                actions.addView(activity.pill("Scan", "wallet-scan") { startScan() },
                    LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(12) })
                content.add(actions, 10)
                if (state == "needs_amount") send?.textOrNull("recipient")?.let { recipient ->
                    content.add(activity.label(recipient, 14f, mono = true, key = "wallet-recipient"), 10)
                    send.textOrNull("description")?.let { content.add(activity.label(it, 13f, Palette.SECONDARY), 2) }
                }
                if (state == "needs_amount" || payAmount.text.isNotEmpty()) place(payAmount, 10)
                if (state == "needs_amount" && send?.has("comment_max") == true && !send.isNull("comment_max")) {
                    val most = send.optInt("comment_max")
                    payComment.hint = "Note (optional, up to $most characters)"
                    payComment.filters = arrayOf(android.text.InputFilter.LengthFilter(most))
                    place(payComment, 10)
                }
                if (state != "idle") send?.textOrNull("message")?.let { content.add(activity.label(it, 13f), 8) }
                val starting = wallet.textOrNull("status") != null
                reviewReady = { state != "quoting" && !starting && payInput.text.isNotBlank() }
                content.add(activity.pill(if (state == "quoting") "Preparing…" else "Continue", "wallet-review", primary = true) {
                    hideKeyboard()
                    quote(payInput.text.toString())
                }.also { review = it }.enabled(reviewReady()), 12, -2)
            }
        }
    }

    /** Quotes a payment to `input` with the typed amount and comment. */
    private fun quote(input: String) = bridge.wallet("wallet_quote", "input" to input,
        "amount" to payAmount.text.toString(), "comment" to payComment.text.toString())

    /** One fee speed as a radio row. */
    private fun speed(speed: JSONObject, chosen: Boolean, enabled: Boolean, choose: () -> Unit): View = activity.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, activity.dp(6), 0, activity.dp(6))
        addView(activity.label(if (chosen) "◉" else "○", 16f), LinearLayout.LayoutParams(activity.dp(24), -2))
        addView(activity.label(speed.getString("label"), 13f), LinearLayout.LayoutParams(0, -2, 1f))
        addView(activity.label(speed.getString("fee"), 13f, mono = true))
        isSelected = chosen
        contentDescription = "${speed.getString("label")}, ${speed.getString("fee")}${if (chosen) ", selected" else ""}"
        enabled(enabled)
        setOnClickListener { if (isEnabled) choose() }
    }

    private fun confirm(quote: JSONObject, paying: Boolean, message: String?) {
        val send = bridge.packet?.objectOrNull("wallet")?.objectOrNull("send")
        val box = activity.column().apply {
            setPadding(activity.dp(14), activity.dp(14), activity.dp(14), activity.dp(14))
            background = activity.rounded(0xFF1A1A1A.toInt(), 14f)
            add(activity.label("Send ${quote.getString("amount")}?", 19f, bold = true))
            val person = send?.textOrNull("person")
            person?.let { add(line("To", it), 10) }
            add(line(if (person == null) "To" else quote.textOrNull("to") ?: quote.getString("kind"), quote.getString("destination"), mono = true), 10)
            send?.textOrNull("person_source")?.let { add(activity.label(it, 12f, Palette.SECONDARY), 4) }
            quote.textOrNull("note")?.let { add(line("For", it), 6) }
            quote.textOrNull("comment")?.let { add(line("Note", it), 6) }
            val speeds = quote.optJSONArray("speeds")?.objects() ?: emptyList()
            if (speeds.isNotEmpty()) {
                add(activity.label("How fast", 14f, Palette.SECONDARY), 8)
                val list = activity.column().apply { tag = "wallet-speeds" }
                for (option in speeds) list.add(speed(option, option.optBoolean("chosen"), !paying) {
                    bridge.wallet("wallet_speed", "quote" to quote.getLong("id"), "speed" to option.getString("id"))
                })
                add(list, 2)
            }
            add(line("Amount", quote.getString("amount")), 6)
            add(line("Fee", quote.getString("fee")), 6)
            addDivider(8)
            add(line("Total", quote.getString("total"), bold = true), 8)
            message?.let { add(activity.label(it, 13f, Palette.SECONDARY), 8) }
            add(activity.label("Payments can't be undone.", 13f, Palette.SECONDARY), 8)
            val actions = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
            actions.addView(activity.pill(if (paying) "Sending…" else "Send ${quote.getString("total")}", "wallet-confirm", primary = true) {
                bridge.wallet("wallet_pay", "quote" to quote.getLong("id"))
            }.enabled(!paying))
            actions.addView(View(activity), LinearLayout.LayoutParams(0, 1, 1f))
            actions.addView(activity.pill("Cancel", "wallet-cancel") { bridge.wallet("wallet_send_reset") }.enabled(!paying))
            add(actions, 12)
        }
        content.add(box, 12)
    }

    private fun line(label: String, value: String, mono: Boolean = false, bold: Boolean = false): View = activity.row().apply {
        addView(activity.label(label, 14f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2))
        addView(activity.label(value, 14f, mono = mono, bold = bold, selectable = mono).apply { gravity = Gravity.END },
            LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = activity.dp(12) })
    }

    private fun startScan() {
        activity.withCamera { granted ->
            if (!granted) return@withCamera
            scanning = true; redraw()
            camera.removeAllViews()
            scanner.start(camera, 4096) { result ->
                stopScan()
                result.onSuccess { value ->
                    payInput.setText(value)
                    quote(value)
                }
            }
        }
    }

    private fun stopScan() {
        if (scanning) scanner.stop()
        scanning = false; camera.removeAllViews(); redraw()
    }

    /** The tab is hidden: stop the camera. */
    fun disappeared() { if (scanning) stopScan() }

    // Buy

    private fun buy(wallet: JSONObject) {
        content.add(activity.label("Buy bitcoin", 17f, bold = true), 24)
        content.add(activity.label("Pay with dollars. The provider's page opens in your browser.", 13f, Palette.SECONDARY), 6)
        place(buyAmount, 10)
        val buy = wallet.objectOrNull("buy")
        val busy = buy?.optBoolean("busy") == true
        for (provider in buy?.optJSONArray("providers")?.objects() ?: emptyList()) {
            val row = activity.column().apply {
                setPadding(activity.dp(12), activity.dp(12), activity.dp(12), activity.dp(12))
                background = activity.rounded(0xFF1A1A1A.toInt(), 12f)
                add(activity.label(provider.getString("label"), 16f, bold = true))
                add(activity.label(provider.getString("detail"), 13f, Palette.SECONDARY), 4)
                tag = "wallet-buy-${provider.getString("id")}"; isClickable = true
                contentDescription = "${provider.getString("label")}. ${provider.getString("detail")}"
                setOnClickListener { hideKeyboard(); bridge.wallet("wallet_buy", "provider" to provider.getString("id"), "amount" to buyAmount.text.toString()) }
            }.enabled(!busy && wallet.textOrNull("status") == null)
            content.add(row, 10)
        }
        if (busy) content.add(activity.label("Opening…", 13f, Palette.SECONDARY), 8)
        buy?.textOrNull("error")?.let { content.add(activity.label(it, 13f), 8) }
    }

    // Deposits, history, recovery

    private fun deposits(deposits: List<JSONObject>, claim: JSONObject?, refund: JSONObject?) {
        content.add(activity.label("Bitcoin deposits", 17f, bold = true), 24)
        for (deposit in deposits) {
            val txid = deposit.getString("txid"); val vout = deposit.getInt("vout")
            val box = activity.column().apply {
                setPadding(activity.dp(12), activity.dp(12), activity.dp(12), activity.dp(12))
                background = activity.rounded(0xFF141414.toInt(), 12f)
                val top = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
                top.addView(activity.label(deposit.getString("amount"), 15f), LinearLayout.LayoutParams(0, -2, 1f))
                // A deposit that needs attention offers Claim and Refund.
                if (deposit.optBoolean("actionable", true)) {
                    top.addView(activity.pill("Claim", "wallet-claim-$txid") { bridge.wallet("wallet_claim_quote", "txid" to txid, "vout" to vout) }
                        .enabled(claim?.optBoolean("busy") != true))
                    top.addView(activity.pill("Refund", "wallet-refund") {
                        refundAddress.setText(""); refundSpeed = "medium"
                        bridge.wallet("wallet_refund_start", "txid" to txid, "vout" to vout)
                    }.enabled(refund?.optBoolean("busy") != true), LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
                }
                add(top)
                add(activity.label(deposit.getString("status"), 13f, Palette.SECONDARY), 4)
                if (claim != null && claim.optString("txid") == txid && claim.optInt("vout") == vout) {
                    if (claim.optBoolean("busy")) add(ProgressBar(activity), 6, -2)
                    claim.textOrNull("quote")?.let { quote ->
                        add(activity.label(quote, 13f), 6)
                        val actions = activity.row()
                        actions.addView(activity.pill("Claim at this fee", "wallet-claim-confirm", primary = true) {
                            bridge.wallet("wallet_claim", "txid" to txid, "vout" to vout) })
                        actions.addView(activity.pill("Cancel") { bridge.wallet("wallet_claim_reset") },
                            LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
                        add(actions, 6)
                    }
                    claim.textOrNull("message")?.let { add(activity.label(it, 13f), 6) }
                }
                if (refund != null && refund.optString("txid") == txid && refund.optInt("vout") == vout) refundView(this, refund)
            }
            content.add(box, 10)
        }
    }

    private fun refundView(box: LinearLayout, refund: JSONObject) {
        val txid = refund.getString("txid"); val vout = refund.getInt("vout")
        val busy = refund.optBoolean("busy")
        val speeds = refund.optJSONArray("speeds")?.objects() ?: emptyList()
        val review = refund.textOrNull("review")
        box.add(activity.label("Refund on-chain", 15f, bold = true), 10)
        if (busy) box.add(ProgressBar(activity), 6, -2)
        if (speeds.isNotEmpty() && review == null) {
            (refundAddress.parent as? android.view.ViewGroup)?.removeView(refundAddress)
            box.add(refundAddress, 8)
            for (option in speeds) box.add(speed(option, option.getString("id") == refundSpeed, true) {
                refundSpeed = option.getString("id"); redraw()
            })
            val actions = activity.row()
            actions.addView(activity.pill("Review refund", "wallet-refund-review", primary = true) {
                if (refundAddress.text.isNotBlank()) bridge.wallet("wallet_refund_review", "txid" to txid, "vout" to vout,
                    "address" to refundAddress.text.toString().trim(), "speed" to refundSpeed)
            })
            actions.addView(activity.pill("Cancel") { bridge.wallet("wallet_refund_reset") },
                LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
            box.add(actions, 8)
        }
        review?.let {
            box.add(activity.label(it, 13f), 8)
            val actions = activity.row()
            actions.addView(activity.pill("Refund", "wallet-refund-confirm", primary = true) {
                bridge.wallet("wallet_refund", "txid" to txid, "vout" to vout) }.enabled(!busy))
            actions.addView(activity.pill("Cancel") { bridge.wallet("wallet_refund_reset") },
                LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
            box.add(actions, 8)
        }
        refund.textOrNull("message")?.let { message ->
            box.add(activity.label(message, 13f), 8)
            if (review == null && speeds.isEmpty() && !busy) box.add(activity.pill("Close") { bridge.wallet("wallet_refund_reset") }, 6, -2)
        }
    }

    /** The unilateral-exit backup, exported to a file the person picks. */
    private fun backup(backup: JSONObject) {
        content.add(activity.label(backup.getString("title"), 17f, bold = true, key = "wallet-backup"), 24)
        content.add(activity.label(backup.getString("detail"), 13f, Palette.SECONDARY), 6)
        val saved = if (backup.has("saved_at") && !backup.isNull("saved_at")) "Saved on this phone " +
            DateUtils.getRelativeTimeSpanString(backup.getLong("saved_at") * 1000, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS)
            else "Not saved yet. It saves after the wallet syncs."
        content.add(activity.label(saved, 13f), 6)
        (backup.textOrNull("error") ?: exportError)?.let { content.add(activity.label(it, 13f), 6) }
        content.add(activity.pill("Export to a file", "wallet-export-exit") {
            exportError = null
            bridge.walletExitExport { name, text, error ->
                if (text == null || name == null) { exportError = error ?: "The backup could not be read."; redraw() }
                else { pendingExport = text; exporter.launch(name) }
            }
        }.enabled(backup.optBoolean("can_export")), 10, -2)
    }

    /** One payment; `method` adds how it traveled, for the full history. */
    private fun paymentRow(payment: JSONObject, method: Boolean): View = activity.row().apply {
        val status = payment.optString("status")
        val when_ = DateUtils.getRelativeTimeSpanString(payment.optLong("at") * 1000, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS)
        val detail = listOfNotNull(if (method) payment.optString("method") else null, if (status == "completed") null else status.replaceFirstChar { it.uppercase() },
            when_.toString()).joinToString(" · ")
        val left = activity.column().apply {
            add(activity.label(payment.getString("title"), 15f))
            add(activity.label(detail, 12f, Palette.SECONDARY), 2)
        }
        addView(left, LinearLayout.LayoutParams(0, -2, 1f))
        val right = activity.column().apply {
            gravity = Gravity.END
            add(activity.label(payment.getString("amount"), 14f).apply { gravity = Gravity.END })
            payment.textOrNull("fee")?.let { add(activity.label(it, 12f, Palette.SECONDARY).apply { gravity = Gravity.END }, 2) }
        }
        addView(right, LinearLayout.LayoutParams(-2, -2))
        contentDescription = "${payment.getString("title")}, ${payment.getString("amount")}, $detail"
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
    }

    private fun recovery(wallet: JSONObject) {
        content.add(activity.label("Recovery", 17f, bold = true), 24)
        content.add(activity.label("Your recovery words restore this wallet on another phone or a computer. Write them down and keep them offline.",
            13f, Palette.SECONDARY), 6)
        val actions = activity.row()
        actions.addView(activity.pill("Show recovery words", "wallet-show-words") { confirmWords() }
            .enabled(wallet.optBoolean("can_show_words")))
        actions.addView(activity.pill("Restore", "wallet-restore") { restore(wallet.optBoolean("empty") == false && !wallet.optBoolean("balance_unknown")) },
            LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(12) })
        content.add(actions, 10)
    }

    private fun updated(seconds: Long?): String {
        seconds ?: return "Not updated yet"
        return "Updated " + DateUtils.getRelativeTimeSpanString(seconds * 1000, System.currentTimeMillis(), DateUtils.SECOND_IN_MILLIS)
    }

    private fun hideKeyboard() {
        activity.currentFocus?.let { focus ->
            activity.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(focus.windowToken, 0)
            focus.clearFocus()
        }
    }

    // Dialogs

    private fun dialog() = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    /** The trust note, opened from the info button. Closing it acknowledges it. */
    fun showTrust() {
        val trust = bridge.packet?.objectOrNull("wallet")?.objectOrNull("trust") ?: return
        val body = activity.column().apply {
            setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(8))
            trust.textOrNull("summary")?.let {
                add(activity.label(it, 16f), 10)
                add(activity.label("Details", 15f, bold = true), 16)
            }
            for (line in trust.optJSONArray("lines").strings()) add(activity.label(line, 13f, Palette.SECONDARY), 10)
        }
        dialog().setTitle(trust.getString("title"))
            .setView(ScrollView(activity).apply { addView(body); tag = "wallet-trust" })
            .setPositiveButton("Done") { _, _ -> if (!trust.optBoolean("acknowledged")) bridge.wallet("wallet_acknowledge") }
            .setOnCancelListener { if (!trust.optBoolean("acknowledged")) bridge.wallet("wallet_acknowledge") }
            .show()
    }

    private fun confirmWords() {
        dialog().setTitle("Show your recovery words?")
            .setMessage("Anyone who sees these words can take your bitcoin. Make sure no one is watching and nothing is recording your screen.")
            .setPositiveButton("Show words") { _, _ -> bridge.walletWords { showWords(it) } }
            .setNegativeButton("Cancel", null)
            .show()
    }

    /** The recovery words, numbered. They exist only while this dialog is open. */
    private fun showWords(words: List<String>) {
        var list: List<String>? = words
        val grid = activity.column().apply { tag = "wallet-words" }
        val half = (words.size + 1) / 2
        for (i in 0 until half) {
            val row = activity.row()
            for (index in listOf(i, i + half)) if (index < words.size) row.addView(
                activity.label("${index + 1}. ${words[index]}", 16f, mono = true), LinearLayout.LayoutParams(0, -2, 1f))
            grid.add(row, 8)
        }
        val body = activity.column().apply {
            setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(8))
            add(activity.label("Write these ${words.size} words down in order and keep them somewhere safe and offline. Anyone with them can take your bitcoin.", 15f))
            add(grid, 12)
            add(activity.pill("Copy words", "wallet-copy-words") {
                // One line of words separated by spaces, the form a restore
                // takes; marked sensitive and cleared after a minute.
                list?.let { Clipboard.copy(activity, "Recovery words", it.joinToString(" "), secret = true) }
            }, 16, -2)
            add(activity.label("The copy is marked sensitive and cleared after a minute. Paste it somewhere offline, not into a message or a notes app that syncs.",
                12f, Palette.SECONDARY), 8)
        }
        val shown = dialog().setTitle("Recovery words").setView(ScrollView(activity).apply { addView(body) })
            // The person wrote them down: the Back up card goes away.
            .setPositiveButton("I wrote them down") { _, _ -> bridge.wallet("wallet_words_saved") }
            .setNegativeButton("Done", null).create()
        // No screenshots or recents thumbnail while the words show.
        secure(shown)
        shown.setOnDismissListener { list = null; grid.removeAllViews() }
        shown.show()
    }

    /**
     * No screenshots, screen recording, or recents thumbnail while recovery
     * words show or are typed. Debug builds skip it with the
     * `allow_secret_captures` launch extra, for emulator captures.
     */
    private fun secure(dialog: AlertDialog) {
        if (BuildConfig.DEBUG && activity.intent.getBooleanExtra("allow_secret_captures", false)) return
        dialog.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
    }

    /** Restore from recovery words. Rust checks them; the field is cleared once they are sent. */
    private fun restore(hasBalance: Boolean) {
        val words = EditText(activity).apply {
            hint = "Recovery words"; tag = "wallet-restore-words"; contentDescription = "Recovery words"
            setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
            // Visible while typed, with no suggestions, learning, or autofill.
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
            importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            isSaveEnabled = false; minLines = 3; maxLines = 8
        }
        val error = activity.label("", 13f, key = "wallet-restore-error").apply { visibility = View.GONE }
        val body = activity.column().apply {
            setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(8))
            add(activity.label("Enter your 12 or 24 recovery words, separated by spaces. This replaces the wallet on this phone.", 15f))
            if (hasBalance) add(activity.label("This wallet holds bitcoin. Write down its recovery words before you replace it, or you lose that bitcoin.", 13f).apply {
                setPadding(activity.dp(10), activity.dp(10), activity.dp(10), activity.dp(10)); background = activity.rounded(0xFF1F1F1F.toInt(), 10f)
            }, 10)
            add(words, 12)
            add(error, 8)
        }
        val shown = dialog().setTitle("Restore").setView(body)
            .setPositiveButton("Restore wallet", null)
            .setNegativeButton("Cancel") { _, _ -> words.setText("") }
            .create()
        secure(shown)
        shown.setOnShowListener {
            shown.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener {
                if (words.text.isBlank()) return@setOnClickListener
                val go = {
                    bridge.restoreWallet(words.text.toString()) { failure ->
                        if (failure == null) { words.setText(""); shown.dismiss() }
                        else { error.text = failure; error.visibility = View.VISIBLE }
                    }
                }
                if (hasBalance) dialog().setTitle("Replace this wallet?")
                    .setMessage("Its bitcoin can only be recovered with its own recovery words.")
                    .setPositiveButton("Replace") { _, _ -> go() }.setNegativeButton("Cancel", null).show()
                else go()
            }
        }
        shown.setOnDismissListener { words.setText("") }
        shown.show()
    }
}
