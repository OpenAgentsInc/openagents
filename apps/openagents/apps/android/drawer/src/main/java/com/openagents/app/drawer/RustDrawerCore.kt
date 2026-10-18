package com.openagents.app.drawer

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.openagents_core.ChatIndicator
import uniffi.openagents_core.ConnectivityState
import uniffi.openagents_core.CoreClientInterface
import uniffi.openagents_core.CoreSavedHistoryInterface
import uniffi.openagents_core.SavedFilingChange
import uniffi.openagents_core.SavedHistoryAction
import uniffi.openagents_core.SavedLoadState
import uniffi.openagents_core.SessionHandle
import uniffi.openagents_core.SessionRow

/** One instance per account. The shell supplies the Rust-selected navigation key
 * and owns the returned session handle. Dispose this adapter on account changes.
 * Regenerate UniFFI after the saved-history Rust additions before compiling it.
 */
class RustDrawerCore(
    private val live: CoreClientInterface,
    private val saved: CoreSavedHistoryInterface,
    private val selected: () -> DrawerKey?,
    private val openLive: (SessionHandle) -> Unit,
    private val openSaved: (String) -> Unit,
) : DrawerCore {
    private val access = Mutex()
    private var savedRequest: DrawerRequest? = null
    private var activeTab = DrawerTab.LIVE

    override suspend fun load(request: DrawerRequest): DrawerFrame = withContext(Dispatchers.IO) { access.withLock {
        activeTab = request.tab
        when (request.tab) {
            DrawerTab.LIVE -> {
                val workspace = live.workspace()
                val connection = live.connectivity().state
                val rows = when {
                    request.query.isNotBlank() -> live.search(request.query, 200u).map { it.session to "Search results" }
                    request.archived -> workspace.archived.map { it to "Archived" }
                    request.projectId != null -> live.project(request.projectId)?.sessions.orEmpty().map { it to "Sessions" }
                    else -> buildList {
                        addAll(workspace.front.pinned.map { it to "Pinned" })
                        workspace.front.sections.forEach { section -> addAll(section.sessions.map { it to section.name }) }
                        addAll(workspace.front.recent.map { it to "Sessions" })
                    }
                }
                DrawerFrame(rows.map { (row, group) -> liveRow(row, group, workspace.pinsReady) }, selected(), false,
                    offline = connection == ConnectivityState.OFFLINE,
                    reconnecting = connection == ConnectivityState.RECONNECTING)
            }
            DrawerTab.SAVED -> {
                val previous = savedRequest
                if (previous == null || previous.projectId != request.projectId) {
                    saved.action(SavedHistoryAction.SelectProject(request.projectId))
                }
                if (previous?.query != request.query || previous?.projectId != request.projectId) {
                    saved.action(SavedHistoryAction.Search(request.query))
                }
                if (saved.snapshot().archived != request.archived) saved.action(SavedHistoryAction.ToggleArchived)
                savedRequest = request
                val snapshot = saved.snapshot()
                val state = snapshot.state
                DrawerFrame(snapshot.rows.map { row ->
                    DrawerRow(DrawerKey(DrawerOrigin.SAVED, row.id), row.title, row.subtitle, row.group,
                        row.agentName, row.indicator?.let(::status), row.unseen, row.pinned, row.archived,
                        if (row.canFile) setOf(DrawerAction.PIN, DrawerAction.ARCHIVE, DrawerAction.RENAME) else emptySet())
                }, selected(), snapshot.hasMore, loading = state is SavedLoadState.Loading,
                    offline = state is SavedLoadState.Failed && state.stale,
                    error = (state as? SavedLoadState.Failed)?.message)
            }
        }
    } }

    override suspend fun send(intent: DrawerIntent): Unit = withContext(Dispatchers.IO) { access.withLock {
        when (intent) {
            is DrawerIntent.Open -> when (intent.key.origin) {
                DrawerOrigin.LIVE -> {
                    val handle = live.openSession(intent.key.id)
                    withContext(Dispatchers.Main) { openLive(handle) }
                    live.markSeen(intent.key.id)
                }
                DrawerOrigin.SAVED -> {
                    saved.action(SavedHistoryAction.Open(intent.key.id))
                    withContext(Dispatchers.Main) { openSaved(intent.key.id) }
                }
            }
            is DrawerIntent.Pin -> if (intent.key.origin == DrawerOrigin.LIVE) {
                if (intent.pinned) live.pinSession(intent.key.id) else live.unpinSession(intent.key.id)
            } else saved.fileChat(intent.key.id, SavedFilingChange.Pin(intent.pinned))
            is DrawerIntent.Archive -> if (intent.key.origin == DrawerOrigin.LIVE) {
                if (intent.archived) live.archiveSession(intent.key.id) else live.unarchiveSession(intent.key.id)
            } else saved.fileChat(intent.key.id, SavedFilingChange.Archive(intent.archived))
            is DrawerIntent.Rename -> if (intent.key.origin == DrawerOrigin.LIVE) {
                live.renameSession(intent.key.id, intent.title)
            } else saved.fileChat(intent.key.id, SavedFilingChange.Rename(intent.title))
            is DrawerIntent.MovePin -> {
                require(intent.key.origin == DrawerOrigin.LIVE) { "Saved pin ordering needs a server contract" }
                require(intent.after?.origin != DrawerOrigin.SAVED && intent.before?.origin != DrawerOrigin.SAVED)
                live.movePin(intent.key.id, intent.after?.id, intent.before?.id)
            }
            DrawerIntent.More -> if (activeTab == DrawerTab.SAVED) saved.action(SavedHistoryAction.MoreChats)
            DrawerIntent.Refresh -> if (activeTab == DrawerTab.SAVED) saved.action(SavedHistoryAction.RefreshChats)
        }
    } }

    override suspend fun restore(key: DrawerKey) = send(DrawerIntent.Open(key))

    private fun liveRow(row: SessionRow, group: String, pinsReady: Boolean) = DrawerRow(
        DrawerKey(DrawerOrigin.LIVE, row.id), row.title, row.preview, group, row.agentLine,
        status(row.indicator), row.unseen, row.pinned, row.archived,
        buildSet {
            add(DrawerAction.ARCHIVE)
            add(DrawerAction.RENAME)
            if (pinsReady) add(DrawerAction.PIN)
        },
    )

    private fun status(value: ChatIndicator): String? = when (value) {
        ChatIndicator.WORKING -> "Working"
        ChatIndicator.AWAITING_INPUT -> "Waiting for you"
        ChatIndicator.ERRORED -> "Couldn't finish"
        ChatIndicator.COMPLETED -> "Finished"
        ChatIndicator.IDLE -> null
    }
}
