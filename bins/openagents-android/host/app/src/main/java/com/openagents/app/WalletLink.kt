// A computer's ask for the owner's wallet (`openagents wallet link`). Rust
// reads the ask from the computer and derives the code the computer shows;
// this host names the computer and the code, asks for the screen lock, and
// sends the owner's tap. Rust seals the wallet to the computer's one-time
// key; the seed never passes through Kotlin. It follows the iOS
// `WalletLink.swift`.
package com.openagents.app

import android.app.Activity
import android.app.AlertDialog
import android.app.KeyguardManager
import android.view.Gravity
import android.view.View
import androidx.activity.result.contract.ActivityResultContracts
import org.json.JSONObject

class WalletLink(private val activity: MainActivity, private val bridge: MobileBridge) {
    private var dialog: AlertDialog? = null
    private var shown: String? = null
    private var failure: String? = null
    private var notice: AlertDialog? = null
    private var authenticating: Pair<String, String>? = null
    private val unlock = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val ask = authenticating; authenticating = null
        if (ask == null) return@registerForActivityResult
        if (result.resultCode == Activity.RESULT_OK) bridge.wallet("wallet_link_approve", "host" to ask.first, "id" to ask.second)
        else { failure = "Not approved. Your wallet stayed on this phone."; shown = null; update(bridge.packet) }
    }

    /** Shows, refreshes, or closes the sheet from the app packet (`wallet_link`). */
    fun update(packet: JSONObject?) {
        val link = packet?.objectOrNull("wallet_link")
        showNotice(link?.textOrNull("notice"))
        val sheet = link?.objectOrNull("sheet")
        // An agent's payment request goes first.
        if (link == null || sheet == null || packet?.objectOrNull("spend")?.objectOrNull("sheet") != null) { close(); return }
        val key = "${sheet}|${link.optBoolean("busy")}|$failure"
        if (key == shown && dialog?.isShowing == true) return
        shown = key
        dialog?.setOnDismissListener(null); dialog?.dismiss()
        dialog = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Use your wallet on ${sheet.getString("computer")}?")
            .setView(body(sheet, link.optBoolean("busy")))
            .setCancelable(false)
            .create().also { it.show() }
    }

    private fun showNotice(text: String?) {
        if (text == null) { notice?.dismiss(); notice = null; return }
        if (notice?.isShowing == true) return
        notice = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setMessage(text)
            .setPositiveButton("OK") { _, _ -> bridge.wallet("wallet_link_dismiss") }
            .setCancelable(false)
            .create().also { it.show() }
    }

    private fun close() {
        dialog?.dismiss(); dialog = null; shown = null; failure = null
    }

    private fun body(sheet: JSONObject, busy: Boolean): View = activity.column().apply {
        setPadding(activity.dp(24), activity.dp(8), activity.dp(24), activity.dp(12))
        tag = "wallet-link-sheet"
        val computer = sheet.getString("computer")
        val code = sheet.getString("code")
        add(activity.label(code, 34f, bold = true, key = "wallet-link-code").apply { gravity = Gravity.CENTER })
        add(activity.label("Shown on $computer", 14f, Palette.SECONDARY).apply { gravity = Gravity.CENTER }, 2)
        add(activity.label("Approve only if $computer shows the code $code. It will hold your wallet and can spend from it, like this phone.",
            13f, Palette.SECONDARY), 16)
        failure?.let { add(activity.label(it, 13f, Palette.FAILURE), 12) }
        add(activity.pill(if (busy) "Sending…" else "Approve", "wallet-link-approve", primary = true) { approve(sheet) }
            .enabled(!busy), 16)
        add(activity.pill("Deny", "wallet-link-deny") {
            bridge.wallet("wallet_link_deny", "host" to sheet.getString("host"), "id" to sheet.getString("id"))
        }.enabled(!busy), 8)
    }

    /** Sending the wallet to a computer always asks for the screen lock first. */
    private fun approve(sheet: JSONObject) {
        failure = null
        val keyguard = activity.getSystemService(KeyguardManager::class.java)
        @Suppress("DEPRECATION")
        val intent = keyguard?.takeIf { it.isDeviceSecure }?.createConfirmDeviceCredentialIntent(
            "Use your wallet on a computer", "Use your wallet on ${sheet.getString("computer")}")
        if (intent == null) {
            failure = "Set a screen lock on this phone to use your wallet on a computer."
            shown = null; update(bridge.packet); return
        }
        authenticating = sheet.getString("host") to sheet.getString("id")
        unlock.launch(intent)
    }
}
