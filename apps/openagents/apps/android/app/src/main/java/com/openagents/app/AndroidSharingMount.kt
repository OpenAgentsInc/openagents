package com.openagents.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.accessibility.NativeSemantics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.CoreSharing

/** The existing account-leased Rust sharing form owns projects, permissions and commands. */
internal class AndroidSharingMount(
    context: Context,
    private val container: ViewGroup,
    private val core: CoreSharing,
    private val work: CoroutineScope,
    semantics: NativeSemantics,
    private val failure: () -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key -> work.launch {
            try {
                val copied = core.activate(instance, revision.toULong(), key)
                main.post { if (live) {
                    if (copied != null) clipboard.setPrimaryClip(ClipData.newPlainText("OpenAgents", copied))
                    renderSafely()
                } }
            } catch (_: Exception) { main.post { if (live) failure() } }
        } },
        changeField = { instance, revision, key, value ->
            try { core.changeField(instance, revision.toULong(), key, value); render() }
            catch (_: Exception) { if (live) failure() }
        }, acknowledge = { _, _ -> }, semantics = semantics,
    )
    init {
        try { core.setVisible(true); render() }
        catch (error: Exception) { close(); throw error }
    }
    fun render() { if (live) adapter.mount(container, core.snapshot().toByteArray(Charsets.UTF_8)) }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        try { core.setVisible(false) }
        finally { try { adapter.dispose() } finally { core.destroy() } }
    }
}
