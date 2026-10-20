package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.accessibility.NativeSemantics
import com.openagents.app.onboarding.DeviceAuthentication
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.CoreBrowserPayments

/**
 * More > Payments from your browser (#12310): native mounting, the device check before a large Pay, and a
 * 10-second refresh while shown. Rust owns the grant, every check, the payment, and the receipt.
 */
internal class AndroidBrowserPaymentsMount(
    context: Context,
    private val container: ViewGroup,
    private val core: CoreBrowserPayments,
    private val work: CoroutineScope,
    private val auth: DeviceAuthentication,
    semantics: NativeSemantics,
    private val failure: () -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private var check: AutoCloseable? = null
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key -> press(instance, revision, key) },
        changeField = { _, _, _, _ -> }, acknowledge = { _, _ -> }, semantics = semantics,
    )
    private val poll = object : Runnable {
        override fun run() {
            if (!live) return
            work.launch {
                try { core.refresh() } catch (_: Exception) { /* The card shows what went wrong. */ }
                main.post { if (live) renderSafely() }
            }
            main.postDelayed(this, 10_000)
        }
    }
    init {
        try { render(); main.post(poll) } catch (error: Exception) { close(); throw error }
    }
    private fun press(instance: String, revision: Long, key: String) {
        val run = { work.launch {
            try { core.activate(instance, revision.toULong(), key) } catch (_: Exception) { /* The card shows it. */ }
            main.post { if (live) renderSafely() }
        } }
        if (!core.needsCheck(key)) { run(); return }
        check?.close()
        check = auth.authenticate("Pay what your browser asked for", "Confirm it's you to pay more than 1,000 sats.") { ok ->
            main.post { if (live && ok) run() }
        }
    }
    fun render() { if (live) adapter.mount(container, core.snapshot().toByteArray(Charsets.UTF_8)) }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        check?.close()
        try { adapter.dispose() } finally { core.destroy() }
    }
}
