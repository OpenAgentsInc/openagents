package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.accessibility.NativeSemantics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.CorePublicProfile

/** More > Public profile (#12308): native mounting only; Rust owns the form, the key's signature, and the name. */
internal class AndroidProfileMount(
    context: Context,
    private val container: ViewGroup,
    private val core: CorePublicProfile,
    private val work: CoroutineScope,
    semantics: NativeSemantics,
    private val failure: () -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key -> work.launch {
            try { core.activate(instance, revision.toULong(), key) } catch (_: Exception) { /* The form shows what went wrong. */ }
            main.post { if (live) renderSafely() }
        } },
        changeField = { instance, revision, key, value ->
            try { core.changeField(instance, revision.toULong(), key, value); render() }
            catch (_: Exception) { if (live) renderSafely() }
        }, acknowledge = { _, _ -> }, semantics = semantics,
    )
    init {
        try { render() } catch (error: Exception) { close(); throw error }
    }
    fun render() { if (live) adapter.mount(container, core.snapshot().toByteArray(Charsets.UTF_8)) }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        try { adapter.dispose() } finally { core.destroy() }
    }
}
