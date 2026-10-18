package com.openagents.app.sessions

import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.SemanticViewAdapter
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.openagents_core.AndroidSessionAction
import uniffi.openagents_core.CoreAndroidSessions
import uniffi.openagents_core.CoreCloudChat

/** Borrows an account-owned Rust object. B1 owns the Activity and transcript mounting. */
class AndroidSessionsHost(
    private val parent: ViewGroup,
    private val owner: CoreAndroidSessions,
    scope: CoroutineScope,
    private val route: (AndroidSessionAction) -> Unit,
    private val failure: (Throwable) -> Unit,
) : AutoCloseable {
    private val job = SupervisorJob(scope.coroutineContext[Job])
    private val tasks = CoroutineScope(scope.coroutineContext + job)
    private val requests = Mutex()
    private var generation = 0L
    private var closed = false
    private var adapter: SemanticViewAdapter? = null

    /** The onboarding hook reuses this exact Rust object; it never creates/resets a chat. */
    fun defaultChat(): CoreCloudChat { check(!closed); return owner.defaultChat() }
    fun openDefaultChat() { ui(); route(AndroidSessionAction.Cloud(defaultChat())) }
    private fun ui() { check(Looper.myLooper() == Looper.getMainLooper()) }
    fun showPicker() {
        ui(); if (closed) return
        val stamp = generation
        tasks.launch {
            try {
                requests.withLock {
                    val bytes = withContext(Dispatchers.IO) { owner.view() }
                    // Keep the request serialized until native mounting and Rust acknowledgment finish.
                    withContext(Dispatchers.Main.immediate) { if (!closed && generation == stamp) mount(bytes) }
                }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (error: Exception) { withContext(Dispatchers.Main.immediate) { if (!closed && generation == stamp) failure(error) } }
        }
    }
    private fun mount(bytes: ByteArray) {
        val renderer = adapter ?: SemanticViewAdapter(parent.context,
            { instance, revision, key -> activate(instance, revision, key) },
            { _, _, _, _ -> },
            { instance, revision -> owner.acknowledge(instance, revision.toULong()) }).also { adapter = it }
        try { renderer.mount(parent, bytes) } catch (error: Exception) { retire(); failure(error) }
    }
    private fun activate(instance: String, revision: Long, key: String) {
        val stamp = generation
        tasks.launch {
            try {
                requests.withLock {
                    val action = owner.activate(instance, revision.toULong(), key)
                    if (action == AndroidSessionAction.Updated) {
                        val bytes = withContext(Dispatchers.IO) { owner.view() }
                        withContext(Dispatchers.Main.immediate) { if (!closed && generation == stamp) mount(bytes) }
                    } else withContext(Dispatchers.Main.immediate) { if (!closed && generation == stamp) route(action) }
                }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (error: Exception) { withContext(Dispatchers.Main.immediate) { if (!closed && generation == stamp) failure(error) } }
        }
    }
    /** Call before Activity/container replacement. The caller retains the Rust owner across recreation. */
    fun retire() { ui(); generation++; job.cancelChildren(); adapter?.dispose(); adapter = null }
    override fun close() { ui(); if (!closed) { retire(); closed = true; job.cancel() } }
}
