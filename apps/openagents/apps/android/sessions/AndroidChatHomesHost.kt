package com.openagents.app.sessions

import android.os.Looper
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import com.openagents.app.SemanticViewAdapter
import com.openagents.app.drawer.DrawerKey
import com.openagents.app.drawer.DrawerOrigin
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.openagents_core.AndroidSessionAction
import uniffi.openagents_core.ChatHomeAction
import uniffi.openagents_core.CoreAndroidSessions
import uniffi.openagents_core.CoreChatHomes
import uniffi.openagents_core.CoreContinuationDraft
import uniffi.openagents_core.CoreSavedHistory
import uniffi.openagents_core.ContinuationDraftAction

internal fun dispatchChatHomeAction(
    action: ChatHomeAction,
    onNavigation: (ChatHomeAction) -> Unit,
    onDraft: (CoreContinuationDraft) -> Unit,
) {
    when (action) {
        is ChatHomeAction.Draft -> onDraft(action.draft)
        else -> onNavigation(action)
    }
}

internal suspend fun continueDraftOnSelected(
    draft: CoreContinuationDraft,
    continueSelected: suspend (CoreContinuationDraft) -> AndroidSessionAction,
    route: (AndroidSessionAction) -> Unit,
) {
    route(continueSelected(draft))
}

/** Mounts Rust-owned chat-home menus in a drawer overlay and borrows account-owned Rust owners. */
class AndroidChatHomesHost(
    private val parent: ViewGroup,
    private val sessions: CoreAndroidSessions,
    private val history: CoreSavedHistory,
    scope: CoroutineScope,
    private val onAction: (ChatHomeAction) -> Unit,
    private val route: (AndroidSessionAction) -> Unit,
    private val failure: (Throwable) -> Unit,
    private val dismiss: () -> Unit,
) : AutoCloseable {
    private val job = SupervisorJob(scope.coroutineContext[Job])
    private val tasks = CoroutineScope(scope.coroutineContext + job)
    private val requests = Mutex()
    private var generation = 0L
    private var closed = false
    private var homes: CoreChatHomes? = null
    private var choices: SemanticViewAdapter? = null
    private var parts: SemanticViewAdapter? = null
    private var activeDraft: CoreContinuationDraft? = null
    private var draftRenderer: SemanticViewAdapter? = null

    /** Opens a live or saved row. Indexed rows have no continuation contract. */
    fun open(key: DrawerKey) {
        ui()
        if (closed) return
        if (key.origin != DrawerOrigin.LIVE && key.origin != DrawerOrigin.SAVED) {
            failure(IllegalArgumentException("This chat cannot be continued."))
            return
        }
        val stamp = replaceMenus()
        tasks.launch {
            try {
                requests.withLock {
                    val next = withContext(Dispatchers.IO) {
                        when (key.origin) {
                            DrawerOrigin.LIVE -> sessions.chatHomes(key.id)
                            DrawerOrigin.SAVED -> {
                                history.openChatPart(key.id)
                                sessions.savedChatHomes(history)
                            }
                            DrawerOrigin.INDEXED -> error("Indexed rows cannot be continued.")
                        }
                    }
                    try {
                        val choiceJson = withContext(Dispatchers.IO) { next.choices() }
                        val partJson = withContext(Dispatchers.IO) { next.parts() }
                        withContext(Dispatchers.Main.immediate) {
                            if (closed || generation != stamp) next.close()
                            else mount(next, choiceJson, partJson, stamp)
                        }
                    } catch (error: Exception) {
                        next.close()
                        throw error
                    }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                withContext(Dispatchers.Main.immediate) {
                    if (!closed && generation == stamp) failure(error)
                }
            }
        }
    }

    private fun mount(next: CoreChatHomes, choiceJson: String, partJson: String, stamp: Long) {
        homes = next
        val columns = LinearLayout(parent.context).apply { orientation = LinearLayout.VERTICAL }
        val choicesContainer = FrameLayout(parent.context)
        val partsContainer = FrameLayout(parent.context)
        columns.addView(choicesContainer, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        columns.addView(partsContainer, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        parent.removeAllViews()
        parent.addView(columns, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        val choiceRenderer = SemanticViewAdapter(parent.context,
            { instance, revision, key -> activate(next, instance, revision, key, stamp) },
            { _, _, _, _ -> }, { _, _ -> })
        val partRenderer = SemanticViewAdapter(parent.context,
            { instance, revision, key -> activate(next, instance, revision, key, stamp) },
            { _, _, _, _ -> }, { _, _ -> })
        try {
            choiceRenderer.mount(choicesContainer, choiceJson.toByteArray(Charsets.UTF_8))
            partRenderer.mount(partsContainer, partJson.toByteArray(Charsets.UTF_8))
            choices = choiceRenderer
            parts = partRenderer
        } catch (error: Exception) {
            choiceRenderer.dispose()
            partRenderer.dispose()
            clearMenus()
            failure(error)
        }
    }

    private fun activate(owner: CoreChatHomes, instance: String, revision: Long, key: String, stamp: Long) {
        tasks.launch {
            var unownedDraft: CoreContinuationDraft? = null
            try {
                val action = requests.withLock {
                    withContext(Dispatchers.IO) {
                        owner.activate(instance, revision.toULong(), key).also { result ->
                            if (result is ChatHomeAction.Draft) unownedDraft = result.draft
                        }
                    }
                }
                withContext(Dispatchers.Main.immediate) {
                    if (closed || generation != stamp) return@withContext
                    dispatchChatHomeAction(action, { navigation ->
                        dismiss()
                        onAction(navigation)
                    }) { draft ->
                        continueOnSelected(draft, stamp)
                        if (unownedDraft === draft) unownedDraft = null
                    }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                withContext(Dispatchers.Main.immediate) {
                    if (!closed && generation == stamp) failure(error)
                }
            } finally {
                withContext(NonCancellable + Dispatchers.Main.immediate) {
                    unownedDraft?.let(::retireDraft)
                    unownedDraft = null
                }
            }
        }
    }

    private fun continueOnSelected(draft: CoreContinuationDraft, stamp: Long) {
        activeDraft?.let(::retireDraft)
        activeDraft = draft
        choices?.dispose()
        choices = null
        parts?.dispose()
        parts = null
        homes?.close()
        homes = null
        tasks.launch {
            try {
                val bytes = withContext(Dispatchers.IO) { draft.view() }
                withContext(Dispatchers.Main.immediate) {
                    if (closed || generation != stamp || activeDraft !== draft) {
                        if (activeDraft === draft) {
                            activeDraft = null
                            retireDraft(draft)
                        }
                        return@withContext
                    }
                    parent.removeAllViews()
                    val renderer = SemanticViewAdapter(
                        parent.context,
                        { instance, revision, key -> activateDraft(draft, instance, revision, key, stamp) },
                        { _, _, _, _ -> },
                        { instance, revision -> draft.acknowledge(instance, revision.toULong()) },
                    )
                    renderer.mount(parent, bytes)
                    draftRenderer = renderer
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                withContext(Dispatchers.Main.immediate) {
                    if (activeDraft === draft) {
                        retireDraft(draft)
                        activeDraft = null
                    }
                    if (!closed && generation == stamp) failure(error)
                }
            }
        }
    }

    private fun activateDraft(
        draft: CoreContinuationDraft,
        instance: String,
        revision: Long,
        key: String,
        stamp: Long,
    ) {
        tasks.launch {
            try {
                val action = requests.withLock {
                    withContext(Dispatchers.IO) {
                        draft.activate(instance, revision.toULong(), key)
                    }
                }
                if (action == ContinuationDraftAction.Updated) {
                    val bytes = withContext(Dispatchers.IO) { draft.view() }
                    withContext(Dispatchers.Main.immediate) {
                        if (!closed && generation == stamp && activeDraft === draft) {
                            draftRenderer?.mount(parent, bytes)
                        }
                    }
                } else {
                    try {
                        continueDraftOnSelected(draft, sessions::continueOnSelected) { result ->
                            if (!closed && generation == stamp && activeDraft === draft) {
                                draftRenderer?.dispose()
                                draftRenderer = null
                                retireDraft(draft)
                                activeDraft = null
                                dismiss()
                                route(result)
                            } else result.destroy()
                        }
                    } finally {
                        val stillOwned = withContext(NonCancellable + Dispatchers.Main.immediate) {
                            if (activeDraft === draft) {
                                draftRenderer?.dispose()
                                draftRenderer = null
                                activeDraft = null
                                true
                            } else false
                        }
                        if (stillOwned) retireDraft(draft)
                    }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                withContext(Dispatchers.Main.immediate) {
                    if (!closed && generation == stamp) failure(error)
                }
            }
        }
    }

    /** Retires only this overlay and its Rust menu handles; sessions and history stay account-owned. */
    override fun close() {
        ui()
        if (closed) return
        closed = true
        generation++
        job.cancelChildren()
        activeDraft?.let(::retireDraft)
        activeDraft = null
        draftRenderer?.dispose()
        draftRenderer = null
        clearMenus()
        job.cancel()
        dismiss()
    }

    private fun replaceMenus(): Long {
        generation++
        job.cancelChildren()
        activeDraft?.let(::retireDraft)
        activeDraft = null
        draftRenderer?.dispose()
        draftRenderer = null
        clearMenus()
        return generation
    }

    private fun clearMenus() {
        choices?.dispose()
        parts?.dispose()
        choices = null
        parts = null
        homes?.close()
        homes = null
        draftRenderer?.dispose()
        draftRenderer = null
        parent.removeAllViews()
    }

    private fun ui() { check(Looper.myLooper() == Looper.getMainLooper()) }

    private fun retireDraft(draft: CoreContinuationDraft) {
        draft.shutdown()
        draft.close()
    }
}
