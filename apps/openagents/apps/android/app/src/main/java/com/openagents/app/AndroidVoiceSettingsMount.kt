package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.ScrollView
import com.openagents.app.accessibility.NativeSemantics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.openagents_core.*

/** Mounts the shared voice-agent preference and execution-host usage views without local choices. */
internal class AndroidVoiceSettingsMount(
    context: Context,
    container: ViewGroup,
    private val owner: CoreAndroidHost,
    private val core: AndroidVoiceSettings,
    private val work: CoroutineScope,
    pickerSemantics: NativeSemantics,
    usageSemantics: NativeSemantics,
    private val failure: () -> Unit,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var live = true
    private val pickerContainer = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private val usageContainer = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private val picker = SemanticViewAdapter(context,
        activate = { instance, revision, key -> work.launch {
            try {
                owner.activateVoiceAgent(core.generation, core.picker, instance, revision.toULong(), key)
                main.post { if (live) renderSafely() }
            } catch (_: Exception) { main.post { if (live) failure() } }
        } }, changeField = { _, _, _, _ -> }, acknowledge = { _, _ -> }, semantics = pickerSemantics)
    private val usage = SemanticViewAdapter(context,
        activate = { instance, revision, key ->
            try { owner.activateVoiceUsage(core.generation, core.usage, instance, revision.toULong(), key); render() }
            catch (_: Exception) { if (live) failure() }
        }, changeField = { _, _, _, _ -> }, acknowledge = { _, _ -> }, semantics = usageSemantics)
    init {
        val body = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(pickerContainer); addView(usageContainer)
        }
        container.addView(ScrollView(context).apply { addView(body) }, ViewGroup.LayoutParams(-1, -1))
        try { core.picker.setVisible(true); render() }
        catch (error: Exception) { close(); throw error }
    }
    fun render() {
        if (!live) return
        owner.syncVoiceUsage(core.generation, core.usage)
        picker.mount(pickerContainer, core.picker.snapshot().toByteArray(Charsets.UTF_8))
        usage.mount(usageContainer, core.usage.viewJson().toByteArray(Charsets.UTF_8))
    }
    private fun renderSafely() { runCatching { render() }.onFailure { if (live) failure() } }
    override fun close() {
        if (!live) return
        live = false
        main.removeCallbacksAndMessages(null)
        core.picker.setVisible(false)
        picker.dispose(); usage.dispose()
        core.picker.destroy(); core.usage.destroy()
    }
}
