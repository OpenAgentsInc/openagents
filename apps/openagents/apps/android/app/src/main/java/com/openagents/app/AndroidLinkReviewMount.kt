package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.accessibility.NativeSemantics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.CoreAndroidLinkReview

/** Native mounting only; Rust owns the link, preview, consent and account fence. */
internal class AndroidLinkReviewMount(
    context: Context,
    private val container: ViewGroup,
    private val core: CoreAndroidLinkReview,
    private val work: CoroutineScope,
    semantics: NativeSemantics,
    private val dismissed: () -> Unit,
    private val failure: () -> Unit,
) : AutoCloseable {
    val id = core.id()
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key ->
            // Process-scoped work completes Rust-owned operations across Activity recreation.
            work.launch {
                try {
                    val close = core.activate(instance, revision.toULong(), key)
                    main.post { if (live) { if (close) dismissed() else render() } }
                } catch (_: Exception) { main.post { if (live) { failure(); renderSafely() } } }
            }
        },
        changeField = { _, _, _, _ -> },
        acknowledge = { instance, revision -> core.acknowledge(instance, revision.toULong()) },
        semantics = semantics,
    )
    fun start() {
        render()
        work.launch {
            try { core.load(); main.post { if (live) renderSafely() } }
            catch (_: Exception) { main.post { if (live) failure() } }
        }
    }
    fun render() { if (live) adapter.mount(container, core.snapshot().toByteArray(Charsets.UTF_8)) }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        adapter.dispose()
    }
}
