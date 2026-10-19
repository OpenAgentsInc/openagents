package com.openagents.app.settings

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.openagents_core.SettingsEffect

/** UI lifetime only. The Rust model owns confirmation, capabilities, and errors. */
class SettingsPresenter(
    private val core: SettingsPort,
    private val scope: CoroutineScope,
    private val mount: (ByteArray) -> Unit,
    private val effect: (SettingsEffect) -> Unit,
    private val unavailable: () -> Unit,
) {
    private var live = true
    private var generation = 0L
    private var read: Job? = null

    fun refresh() {
        if (!live) return
        val ticket = ++generation
        read?.cancel()
        read = scope.launch {
            try {
                val bytes = core.snapshot()
                if (live && ticket == generation) mount(bytes)
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (live && ticket == generation) unavailable() }
        }
    }

    fun activate(instance: String, revision: ULong, key: String) {
        if (!live) return
        scope.launch {
            try {
                val result = core.activate(instance, revision, key)
                if (live) { effect(result); refresh() }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (live) refresh() }
        }
    }

    fun change(instance: String, revision: ULong, key: String, value: String) {
        if (!live) return
        try { core.changeField(instance, revision, key, value); refresh() }
        catch (_: Exception) { refresh() }
    }

    fun refreshCloud() {
        if (!live) return
        scope.launch {
            try { core.refreshCloud(); if (live) refresh() }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (live) refresh() }
        }
    }

    fun wallpaper(ticket: String, uri: String, name: String, permissionSaved: Boolean) {
        if (!live) return
        scope.launch {
            try {
                val result = core.wallpaper(ticket, uri, name, permissionSaved)
                if (live) { effect(result); refresh() }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { if (live) refresh() }
        }
    }

    /** Called on teardown and before rebinding after any account generation change. */
    fun dispose() {
        if (!live) return
        live = false
        generation++
        read?.cancel()
        core.dispose()
    }
}
