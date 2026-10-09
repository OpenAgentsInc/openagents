// The phone on the person's openagents.com account (#11107, #11165): sign in
// with a code, the account's chats, and what Coder runs on the computers.
// Rust owns the state and draws every screen
// (crates/openagents-mobile/src/account_link.rs); this file keeps the
// session in the Keystore-encrypted store, draws the sign-in QR code, and
// shows notifications with Approve and Deny.
package com.openagents.app

import android.Manifest
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.drawable.BitmapDrawable
import android.os.Build
import android.view.Gravity
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import org.json.JSONObject

/** The account surface's page: the menu button, then Rust's view. */
internal class AccountLink(private val activity: Activity, private val bridge: MobileBridge, menu: android.view.View) {
    val root: LinearLayout = activity.column()
    private val content = FrameLayout(activity)
    private val qr = ImageView(activity).apply {
        scaleType = ImageView.ScaleType.FIT_CENTER
        contentDescription = "Sign-in QR code"; tag = "link-qr"
    }
    private var shownQr: String? = null
    private val renderer = NativeRenderer(activity, { view, node -> bridge.activate("link", view, node) },
        { token, value -> bridge.submit("link", token, value) }, floating = true, surfaces = { resource ->
            if (resource == "link-qr") FrameLayout(activity).apply {
                (qr.parent as? android.view.ViewGroup)?.removeView(qr)
                addView(qr, FrameLayout.LayoutParams(activity.dp(220), activity.dp(220), Gravity.CENTER_HORIZONTAL))
                minimumHeight = activity.dp(228)
            } else null
        })
    /** The screen on view (`account`, `chats`, `chat`, or `running`) and its chat. */
    var screen = "account"; private set
    var chat: String? = null; private set

    init {
        val bar = activity.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(6))
            addView(menu, LinearLayout.LayoutParams(activity.dp(44), activity.dp(44)))
        }
        root.addView(bar, LinearLayout.LayoutParams(-1, -2))
        root.addView(content, LinearLayout.LayoutParams(-1, 0, 1f))
    }

    /** Shows `screen` (and opens `chat`). */
    fun show(screen: String, chat: String? = null) {
        this.screen = screen
        this.chat = chat
        if (chat != null) bridge.link("show", "screen" to "chat", "id" to chat)
        else bridge.link("show", "screen" to screen)
    }

    fun hide() = bridge.link("hide")

    fun update(link: JSONObject?) {
        val code = link?.objectOrNull("qr")
        val encoded = code?.toString()
        if (encoded != shownQr) {
            shownQr = encoded
            qr.setImageDrawable(code?.let { qrBitmap(it) }?.let { bitmap ->
                BitmapDrawable(activity.resources, bitmap).apply { isFilterBitmap = false } })
        }
        try { renderer.mount(content, link?.objectOrNull("view")) } catch (problem: Exception) {
            renderer.clear(); content.removeAllViews()
            content.addView(activity.text(problem.message ?: "This screen couldn't be shown.", 14f, Palette.SECONDARY).apply {
                setPadding(activity.dp(16), activity.dp(16), activity.dp(16), activity.dp(16)) })
        }
    }

    companion object {
        private const val CHANNEL = "openagents.agents"
        const val EXTRA_ACTION = "link_action"
        const val EXTRA_COMPUTER = "link_computer"
        const val EXTRA_ITEM = "link_item"
        const val EXTRA_QUESTION = "link_question"
        private var asked = false

        /** Keeps or forgets the session and shows notices, as the packet asks. */
        fun settle(context: Context, link: JSONObject?) {
            link ?: return
            link.objectOrNull("store")?.let { session ->
                DeviceKey.saveAccountSession(context, session.toString())
                (context as? Activity)?.let { ask(it) }
            }
            if (link.optBoolean("forget")) DeviceKey.deleteAccountSession(context)
            for (notice in link.optJSONArray("notify")?.objects().orEmpty()) notify(context, notice)
        }

        /** Asks once for permission to show notices (Android 13 and later). */
        fun ask(activity: Activity) {
            if (asked || Build.VERSION.SDK_INT < 33) return
            asked = true
            if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 7107)
            }
        }

        private fun notify(context: Context, notice: JSONObject) {
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            if (Build.VERSION.SDK_INT >= 26) manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Coder on your computers", NotificationManager.IMPORTANCE_HIGH))
            val computer = notice.optString("computer")
            val item = notice.optString("item")
            val question = notice.textOrNull("question")
            val id = notice.optString("id").hashCode()
            fun intent(action: String, code: Int) = PendingIntent.getActivity(context, code,
                Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
                    .putExtra(EXTRA_ACTION, action).putExtra(EXTRA_COMPUTER, computer)
                    .putExtra(EXTRA_ITEM, item).putExtra(EXTRA_QUESTION, question),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            val builder = (if (Build.VERSION.SDK_INT >= 26) android.app.Notification.Builder(context, CHANNEL)
                else @Suppress("DEPRECATION") android.app.Notification.Builder(context))
                .setSmallIcon(R.drawable.ic_launcher)
                .setContentTitle(notice.optString("title"))
                .setContentText(notice.optString("body"))
                .setStyle(android.app.Notification.BigTextStyle().bigText(notice.optString("body")))
                .setAutoCancel(true)
                .setContentIntent(intent("open", id))
            if (question != null) {
                builder.addAction(android.app.Notification.Action.Builder(null, "Approve", intent("approve", id + 1)).build())
                builder.addAction(android.app.Notification.Action.Builder(null, "Deny", intent("deny", id + 2)).build())
            }
            runCatching { manager.notify(id, builder.build()) }
        }

        /** A notification's tap or button: Approve or Deny answers; a tap opens Running. Returns whether it was one. */
        fun handle(intent: Intent?, bridge: MobileBridge, openRunning: () -> Unit): Boolean {
            val action = intent?.getStringExtra(EXTRA_ACTION) ?: return false
            val computer = intent.getStringExtra(EXTRA_COMPUTER).orEmpty()
            val item = intent.getStringExtra(EXTRA_ITEM).orEmpty()
            val question = intent.getStringExtra(EXTRA_QUESTION)
            intent.removeExtra(EXTRA_ACTION)
            if ((action == "approve" || action == "deny") && question != null) {
                bridge.link("answer", "computer" to computer, "item" to item, "question" to question,
                    "approve" to (action == "approve"))
            }
            openRunning()
            return true
        }
    }
}
