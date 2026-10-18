package com.openagents.app.settings

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.provider.Settings
import uniffi.openagents_core.SettingsEffect
import uniffi.openagents_core.SettingsPreferences

/** Android framework calls only. Register navigation callbacks before advertising
 * their Rust capabilities; project-sharing UI uses its existing Rust Native form.
 */
class AndroidSettingsActions(
    private val activity: Activity,
    private val wallpaperRequestCode: Int,
    private val applyPreferences: (SettingsPreferences) -> Unit,
    private val wallpaper: (ticket: String, uri: String, name: String, permissionSaved: Boolean) -> Unit,
    private val openPairing: (() -> Unit)?,
    private val openArchive: (() -> Unit)?,
    private val openProjectSharing: (() -> Unit)?,
) : AutoCloseable {
    private var pickerTicket: String? = null
    private var live = true

    fun handle(effect: SettingsEffect) {
        if (!live) return
        when (effect) {
            SettingsEffect.None -> Unit
            SettingsEffect.OpenPairing -> requireNotNull(openPairing).invoke()
            SettingsEffect.OpenArchive -> requireNotNull(openArchive).invoke()
            SettingsEffect.OpenProjectSharing -> requireNotNull(openProjectSharing).invoke()
            SettingsEffect.OpenNotificationSettings -> {
                val intent = if (Build.VERSION.SDK_INT >= 26) {
                    Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, activity.packageName)
                } else Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null))
                activity.startActivity(intent)
            }
            is SettingsEffect.PreferencesChanged -> applyPreferences(effect.preferences)
            is SettingsEffect.ShareText -> {
                val send = Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TEXT, effect.text)
                }
                activity.startActivity(Intent.createChooser(send, effect.title))
            }
            is SettingsEffect.ChooseWallpaper -> {
                pickerTicket = effect.ticket
                @Suppress("DEPRECATION")
                activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    type = "image/*"
                    addCategory(Intent.CATEGORY_OPENABLE)
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                }, wallpaperRequestCode)
            }
        }
    }

    /** The containing Activity forwards only this reserved request code. */
    fun onActivityResult(requestCode: Int, resultCode: Int, result: Intent?): Boolean {
        if (!live || requestCode != wallpaperRequestCode) return false
        val ticket = pickerTicket ?: return false
        pickerTicket = null
        val uri = result?.data
        if (resultCode != Activity.RESULT_OK || uri == null) return true
        val saved = try {
            activity.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            true
        } catch (_: SecurityException) { false }
        val name = try {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        } catch (_: Exception) { null }
        wallpaper(ticket, uri.toString(), name ?: "Wallpaper", saved)
        return true
    }

    override fun close() { live = false; pickerTicket = null }
}
