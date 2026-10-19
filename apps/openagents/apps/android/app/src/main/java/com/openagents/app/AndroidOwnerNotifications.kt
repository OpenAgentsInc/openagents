package com.openagents.app

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.app.NotificationCompat
import com.openagents.app.platform.AndroidPushNotifications
import uniffi.openagents_core.OwnerNoteView

/** Android notification presentation for a note already checked by Rust. Taps are rechecked by Rust. */
internal object AndroidOwnerNotifications {
    const val OPEN = "com.openagents.app.OPEN_OWNER_NOTE"
    const val LINK = "owner-note-link"
    const val REQUEST = "owner-note-request"
    private const val TAG = "openagents.owner-note:"
    fun show(context: Context, note: OwnerNoteView): Boolean {
        if (!AndroidPushNotifications.canPostNotifications(context)) return false
        val manager = context.getSystemService(NotificationManager::class.java) ?: return false
        AndroidPushNotifications.createChannels(context)
        val intent = Intent(context, MainActivity::class.java).apply {
            action = OPEN
            // Unique PendingIntent identity, even when several notices refer to the same chat.
            data = Uri.Builder().scheme("openagents").authority("notification").appendPath(note.requestId).build()
            putExtra(LINK, note.deepLink); putExtra(REQUEST, note.requestId)
            addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        }
        val tap = PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        return runCatching {
            manager.notify(TAG + note.requestId, 0, NotificationCompat.Builder(context, AndroidPushNotifications.OWNER_NOTE_CHANNEL)
                .setSmallIcon(android.R.drawable.ic_dialog_info).setContentTitle(note.title)
                .setContentText(note.body).setStyle(NotificationCompat.BigTextStyle().bigText(note.body))
                .setContentIntent(tap).setAutoCancel(true).setOnlyAlertOnce(true).build())
            true
        }.getOrDefault(false)
    }
    fun clear(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        runCatching { manager.activeNotifications.filter { it.tag?.startsWith(TAG) == true }.forEach { manager.cancel(it.tag, it.id) } }
    }
}
