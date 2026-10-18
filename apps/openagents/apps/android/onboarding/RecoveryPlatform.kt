package com.openagents.app.onboarding

import android.app.KeyguardManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.widget.EditText
import android.widget.TextView
import java.util.UUID

/** The B1 ActivityResult owner maps only RESULT_OK to true. The returned handle cancels delivery. */
fun interface CredentialLauncher {
    fun launch(intent: Intent, result: (Boolean) -> Unit): AutoCloseable
}
interface DeviceAuthentication {
    fun authenticate(title: String, description: String, result: (Boolean) -> Unit): AutoCloseable
}
/** Supported device credentials on API26+. Missing lock screen, intent, or launcher fails closed. */
class AndroidDeviceAuthentication(private val context: Context, private val launcher: CredentialLauncher) : DeviceAuthentication {
    @Suppress("DEPRECATION")
    override fun authenticate(title: String, description: String, result: (Boolean) -> Unit): AutoCloseable {
        val manager = context.getSystemService(Context.KEYGUARD_SERVICE) as? KeyguardManager
        val intent = if (manager?.isDeviceSecure == true) manager.createConfirmDeviceCredentialIntent(title, description) else null
        if (intent == null) { result(false); return AutoCloseable {} }
        return try { launcher.launch(intent, result) } catch (_: Exception) { result(false); AutoCloseable {} }
    }
}

/** Owns only its window flag and screen references; no account or recovery data is persisted. */
class SecretScreen(private val window: Window, private val parent: ViewGroup) : AutoCloseable {
    private var ownedSecure = false
    private val previousAutofill = parent.importantForAutofill
    private val previousSave = parent.isSaveEnabled
    private val previousVisibility = parent.visibility
    fun protect() {
        if (window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE == 0) {
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE); ownedSecure = true
        }
        parent.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        parent.isSaveEnabled = false
    }
    fun harden() {
        fun visit(view: View) {
            view.isSaveEnabled = false
            view.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            if (view is TextView && view !is EditText) {
                view.setTextIsSelectable(false); view.isLongClickable = false
                view.customSelectionActionModeCallback = object : android.view.ActionMode.Callback {
                    override fun onCreateActionMode(mode: android.view.ActionMode, menu: android.view.Menu) = false
                    override fun onPrepareActionMode(mode: android.view.ActionMode, menu: android.view.Menu) = false
                    override fun onActionItemClicked(mode: android.view.ActionMode, item: android.view.MenuItem) = false
                    override fun onDestroyActionMode(mode: android.view.ActionMode) {}
                }
            }
            if (view is EditText) {
                val input = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD or android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                if (view.inputType != input) view.inputType = input
                view.imeOptions = view.imeOptions or android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
            }
            if (view is ViewGroup) for (i in 0 until view.childCount) visit(view.getChildAt(i))
        }
        visit(parent)
    }
    fun hide() {
        parent.visibility = View.INVISIBLE
        fun clear(view: View) {
            if (view is TextView) { view.text = ""; view.contentDescription = null }
            if (view is ViewGroup) for (i in 0 until view.childCount) clear(view.getChildAt(i))
        }
        clear(parent)
    }
    fun show() { parent.visibility = View.VISIBLE }
    override fun close() {
        hide()
        if (ownedSecure) window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        ownedSecure = false
        parent.importantForAutofill = previousAutofill
        parent.isSaveEnabled = previousSave
        parent.visibility = previousVisibility
    }
}

interface SecretClipboard : AutoCloseable { fun copy(text: String, clearAfterMs: Long) }

/** The clipboard owns the text after copy. This object keeps only a nonsecret ownership marker. */
class RecoveryClipboard(private val clipboard: ClipboardManager, private val handler: Handler = Handler(Looper.getMainLooper())) : SecretClipboard {
    private var marker: String? = null
    private var expiry: Runnable? = null
    override fun copy(text: String, clearAfterMs: Long) {
        require(clearAfterMs in 1..60_000)
        close()
        val token = UUID.randomUUID().toString()
        val clip = ClipData.newPlainText("Recovery words", text)
        clip.description.extras = PersistableBundle().apply {
            putBoolean("android.content.extra.IS_SENSITIVE", true)
            putString(OWNER, token)
        }
        // Install the deadline before placing secret text on the clipboard.
        marker = token
        val task = Runnable { clearOwned(token) }
        expiry = task
        check(handler.postDelayed(task, clearAfterMs))
        try { clipboard.setPrimaryClip(clip) } catch (error: Exception) { close(); throw error }
    }
    private fun clearOwned(token: String) {
        if (marker != token) return
        val owned = clipboard.primaryClipDescription?.extras?.getString(OWNER) == token
        if (owned) {
            if (Build.VERSION.SDK_INT >= 28) clipboard.clearPrimaryClip()
            else clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
        }
        marker = null; expiry = null
    }
    override fun close() {
        expiry?.let { handler.removeCallbacks(it) }; expiry = null
        marker?.let { clearOwned(it) }
    }
    companion object { private const val OWNER = "com.openagents.app.recovery.clipboard-owner" }
}
