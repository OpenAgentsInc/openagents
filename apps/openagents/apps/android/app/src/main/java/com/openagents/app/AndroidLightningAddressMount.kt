package com.openagents.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.accessibility.NativeSemantics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.json.JSONObject
import uniffi.openagents_core.CoreLightningAddress

/** Mounts the shared Rust wallet sheet and handles its clipboard effect. */
internal class AndroidLightningAddressMount(
    private val context: Context,
    private val container: ViewGroup,
    private val core: CoreLightningAddress,
    private val work: CoroutineScope,
    semantics: NativeSemantics,
    private val failure: () -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key ->
            work.launch {
                val result = runCatching { core.activate(instance, revision.toULong(), key) }
                main.post {
                    if (live) {
                        result.onSuccess { effect ->
                            val value = JSONObject(effect)
                            val text = value.optString("copy")
                            val share = value.optString("share")
                            if (share.isNotEmpty()) {
                                val intent = Intent(Intent.ACTION_SEND).apply { type = "text/plain"; putExtra(Intent.EXTRA_TEXT, share) }
                                context.startActivity(Intent.createChooser(intent, "Share address"))
                            }
                            if (text.isNotEmpty()) {
                                (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                                    .setPrimaryClip(ClipData.newPlainText("Lightning address", text))
                            }
                        }.onFailure { failure() }
                        renderSafely()
                    }
                }
            }
        },
        changeField = { _, _, _, _ -> }, acknowledge = { _, _ -> }, semantics = semantics,
    )
    private val poll = object : Runnable {
        override fun run() {
            if (!live) return
            work.launch {
                runCatching { core.refresh() }
                main.post { if (live) renderSafely() }
            }
            main.postDelayed(this, 10_000)
        }
    }
    init { render(); main.post(poll) }
    fun render() { if (live) adapter.mount(container, core.snapshot().toByteArray(Charsets.UTF_8)) }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        try { adapter.dispose() } finally { core.destroy() }
    }
}
