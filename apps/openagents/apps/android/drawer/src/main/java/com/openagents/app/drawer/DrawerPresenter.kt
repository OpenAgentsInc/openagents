package com.openagents.app.drawer

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** UI request lifetime only; the core owns selection, rows, and their ordering.
 * Call on the UI thread with a UI-dispatched scope. Account changes must call
 * detach before binding a new account-fenced core.
 */
class DrawerPresenter(
    private val core: DrawerCore,
    private val scope: CoroutineScope,
    private val render: (DrawerFrame) -> Unit,
) {
    var request = DrawerRequest()
        private set
    private var generation = 0L
    private var pending: Job? = null
    private var attached = true

    fun show(request: DrawerRequest = this.request) {
        this.request = request
        launch { core.load(request) }
    }

    fun send(intent: DrawerIntent) = launch {
        core.send(intent)
        core.load(request)
    }

    fun restore(key: DrawerKey?, request: DrawerRequest) {
        this.request = request
        launch {
            if (key != null) core.restore(key)
            core.load(request)
        }
    }

    private fun launch(read: suspend () -> DrawerFrame) {
        if (!attached) return
        val ticket = ++generation
        pending?.cancel()
        pending = scope.launch {
            try {
                val frame = read()
                if (attached && ticket == generation) render(frame)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                // Do not reuse rows from a possibly different account.
                if (attached && ticket == generation) {
                    render(DrawerFrame(emptyList(), null, false, error = "Couldn't load chats. Try again."))
                }
            }
        }
    }

    fun detach() {
        attached = false
        generation++
        pending?.cancel()
        pending = null
        render(DrawerFrame(emptyList(), null, false))
    }
}
