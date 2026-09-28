// Agents' payment requests (agent spending, phase 1): an agent on one of the
// owner's computers asks, and the owner approves each payment here. Rust
// reads the requests, checks them against the computer's grant and the
// ledger, and decodes the payee and amount from the invoice itself; this
// host shows the approval sheet, asks for the screen lock above the
// threshold Rust names, and sends the owner's tap. Nothing pays without it.
// It follows the iOS `AgentPayments.swift`.
package com.openagents.app

import android.app.Activity
import android.app.AlertDialog
import android.app.KeyguardManager
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import androidx.activity.result.contract.ActivityResultContracts
import org.json.JSONObject

class AgentPayments(private val activity: MainActivity, private val bridge: MobileBridge) {
    private var dialog: AlertDialog? = null
    /** The request on screen and what its sheet shows, to rebuild only on change. */
    private var shown: String? = null
    private var failure: String? = null
    private var authenticating: String? = null
    private val unlock = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val request = authenticating; authenticating = null
        if (request == null) return@registerForActivityResult
        if (result.resultCode == Activity.RESULT_OK) bridge.spend("spend_approve", "request" to request)
        else { failure = "Not approved. Nothing was paid."; shown = null; update(bridge.packet) }
    }

    /** Shows, refreshes, or closes the approval sheet from the app packet (`spend`). */
    fun update(packet: JSONObject?) {
        val spend = packet?.objectOrNull("spend")
        val sheet = spend?.objectOrNull("sheet")
        if (sheet == null) { close(); return }
        val key = "${sheet}|${spend.optInt("waiting")}|${spend.optBoolean("busy")}|$failure"
        if (key == shown && dialog?.isShowing == true) return
        if (dialog?.isShowing == true && shown?.substringBefore('|') != sheet.toString()) failure = null
        shown = key
        dialog?.setOnDismissListener(null); dialog?.dismiss()
        dialog = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Payment request")
            .setView(ScrollView(activity).apply { addView(body(sheet, spend)) })
            .setCancelable(false)
            .create().also { it.show() }
    }

    private fun close() {
        dialog?.dismiss(); dialog = null; shown = null; failure = null
    }

    private fun body(sheet: JSONObject, spend: JSONObject): View = activity.column().apply {
        setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(12))
        tag = "spend-sheet"
        val busy = spend.optBoolean("busy")
        add(activity.label(sheet.getString("amount"), 34f, bold = true, key = "spend-amount").apply { gravity = Gravity.CENTER })
        add(activity.label("Fee ${sheet.getString("fee")}", 14f, Palette.SECONDARY).apply { gravity = Gravity.CENTER }, 2)
        add(activity.label("ASKED BY", 12f, Palette.SECONDARY, bold = true), 16)
        add(row("Computer", sheet.getString("computer")), 4)
        sheet.textOrNull("title")?.let { add(row("Task", it), 4) }
        add(row("Purpose", sheet.getString("purpose")), 4)
        sheet.textOrNull("resource")?.let { add(row("For", it), 4) }
        sheet.textOrNull("note")?.let { add(row("Agent's note", it), 4) }
        add(activity.label("PAID TO", 12f, Palette.SECONDARY, bold = true), 16)
        add(row("Payee", sheet.getString("payee")), 4)
        if (sheet.optBoolean("payee_new")) add(activity.label("You haven't paid this payee from this computer before.", 13f, 0xFFFF9F0A.toInt()), 4)
        sheet.textOrNull("description")?.let { add(row("Invoice says", it), 4) }
        add(activity.label("Read from the invoice itself, not from the agent's description.", 12f, Palette.SECONDARY), 4)
        add(row("Fee ceiling", sheet.getString("fee_ceiling")), 14)
        add(row("This computer has", sheet.getString("remaining")), 4)
        val waiting = spend.optInt("waiting")
        if (waiting > 0) add(row("Also waiting", "$waiting more"), 4)
        failure?.let { add(activity.label(it, 13f, Palette.FAILURE), 12) }
        add(activity.pill(if (busy) "Paying…" else "Approve and pay ${sheet.getString("amount")}", "spend-approve", primary = true) {
            approve(sheet)
        }.enabled(sheet.optBoolean("ready") && !busy), 16)
        add(activity.pill("Deny", "spend-deny") { bridge.spend("spend_deny", "request" to sheet.getString("request")) }.enabled(!busy), 8)
        add(activity.label("Stop payment requests from ${sheet.getString("computer")}", 14f, Palette.FAILURE, key = "spend-block").apply {
            gravity = Gravity.CENTER; setPadding(0, activity.dp(14), 0, activity.dp(6))
            enabled(!busy)
            setOnClickListener {
                AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                    .setTitle("Stop ${sheet.getString("computer")}'s payment requests?")
                    .setMessage("Its waiting requests are refused. You can allow it again from the Wallet tab.")
                    .setPositiveButton("Stop requests") { _, _ -> bridge.spend("spend_block", "host" to sheet.getString("host")) }
                    .setNegativeButton("Cancel", null).show()
            }
        }, 4)
    }

    private fun row(label: String, value: String): View = activity.row().apply {
        addView(activity.label(label, 14f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2))
        addView(activity.label(value, 14f, selectable = true).apply { gravity = Gravity.END },
            LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = activity.dp(12) })
    }

    /** The screen lock first when Rust asks for it; then the tap. */
    private fun approve(sheet: JSONObject) {
        failure = null
        val request = sheet.getString("request")
        if (!sheet.optBoolean("authenticate")) { bridge.spend("spend_approve", "request" to request); return }
        val keyguard = activity.getSystemService(KeyguardManager::class.java)
        if (keyguard == null || !keyguard.isDeviceSecure) {
            failure = "Set a screen lock on this phone to approve larger payments."; shown = null; update(bridge.packet); return
        }
        @Suppress("DEPRECATION")
        val intent = keyguard.createConfirmDeviceCredentialIntent("Approve payment",
            "Pay ${sheet.getString("amount")} for ${sheet.getString("computer")}") ?: run {
            bridge.spend("spend_approve", "request" to request); return
        }
        authenticating = request
        unlock.launch(intent)
    }

    companion object {
        /** The Wallet tab's list of agent payments and the computers that may ask. */
        fun section(activity: MainActivity, content: LinearLayout, spend: JSONObject, bridge: MobileBridge) {
            val computers = spend.optJSONArray("computers")?.objects() ?: emptyList()
            val history = spend.optJSONArray("history")?.objects() ?: emptyList()
            if (computers.isEmpty() && history.isEmpty()) return
            val box = activity.column().apply {
                setPadding(activity.dp(14), activity.dp(14), activity.dp(14), activity.dp(14))
                background = activity.rounded(0x0FFFFFFF, 12f)
                tag = "agent-payments"
                add(activity.label("Agent payments", 17f, bold = true))
                spend.textOrNull("notice")?.let { notice ->
                    val row = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
                    row.addView(activity.label(notice, 13f), LinearLayout.LayoutParams(0, -2, 1f))
                    row.addView(activity.label("OK", 14f, Palette.LINK).apply {
                        setPadding(activity.dp(12), activity.dp(8), 0, activity.dp(8)); setOnClickListener { bridge.spend("spend_dismiss") }
                    })
                    add(row, 8)
                }
                for (computer in computers) {
                    val row = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
                    row.addView(activity.column().apply {
                        add(activity.label(computer.getString("computer"), 15f))
                        add(activity.label(computer.getString("remaining"), 12f, Palette.SECONDARY), 2)
                    }, LinearLayout.LayoutParams(0, -2, 1f))
                    if (computer.optBoolean("blocked")) row.addView(activity.label("Allow", 14f, Palette.LINK).apply {
                        setPadding(activity.dp(12), activity.dp(8), 0, activity.dp(8))
                        setOnClickListener { bridge.spend("spend_allow", "host" to computer.getString("host")) }
                    })
                    add(row, 10)
                }
                for (entry in history) {
                    val row = activity.row()
                    row.addView(activity.column().apply {
                        add(activity.label(entry.textOrNull("title") ?: entry.getString("purpose"), 15f))
                        add(activity.label(entry.textOrNull("detail") ?: "${entry.getString("computer")} · ${entry.getString("purpose")}",
                            12f, Palette.SECONDARY), 2)
                    }, LinearLayout.LayoutParams(0, -2, 1f))
                    val state = entry.getString("state")
                    row.addView(activity.column().apply {
                        gravity = Gravity.END
                        add(activity.label(entry.getString("amount"), 14f).apply { gravity = Gravity.END })
                        add(activity.label(state.replaceFirstChar { it.uppercase() }, 12f,
                            if (state == "paid") Palette.SUCCESS else Palette.SECONDARY).apply { gravity = Gravity.END }, 2)
                    })
                    add(row, 10)
                }
            }
            content.add(box, 24)
        }
    }
}
