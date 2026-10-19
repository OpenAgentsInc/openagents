package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.CoreLabActions

/** Borrows the retained Rust form; UI teardown leaves its draft with the Rust host. */
internal class AndroidLabActionsMount(
    context: Context,
    private val container: ViewGroup,
    private val core: CoreLabActions,
    private val work: CoroutineScope,
    private val failure: () -> Unit,
) : AutoCloseable {
    private var live = true
    private val main = Handler(Looper.getMainLooper())
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, node -> work.launch {
            try { core.activate(instance, revision.toULong(), node) }
            catch (_: Exception) { main.post { if (live) failure() } }
        } }, changeField = { instance, revision, node, value ->
            try { core.changeField(instance, revision.toULong(), node, value) }
            catch (_: Exception) { if (live) failure() }
        }, acknowledge = { instance, revision -> core.acknowledge(instance, revision.toULong()) },
    )
    fun render() {
        if (live) adapter.mount(container, core.frame().json)
    }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        adapter.dispose()
        core.destroy()
    }
}
