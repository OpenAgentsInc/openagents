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
import uniffi.openagents_core.SavedIndexedTranscript
import uniffi.openagents_core.SavedPinMove
import uniffi.openagents_core.DrawerPinHints
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
    private val openIndexed: (SavedIndexedTranscript) -> Unit,
) : DrawerCore {
    private val access = Mutex()
    private var savedRequest: DrawerRequest? = null
    private var activeTab = DrawerTab.LIVE

    override suspend fun load(request: DrawerRequest): DrawerFrame = withContext(Dispatchers.IO) {
        access.withLock {
            activeTab = request.tab
            when (request.tab) {
                DrawerTab.LIVE -> {
                    val workspace = live.workspace()
                    val connection = live.connectivity().state
                    val moves = live.drawerPinMoves().associateBy { it.id }
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
                    DrawerFrame(rows.map { (row, group) -> liveRow(row, group, workspace.pinsReady, moves[row.id]) }, selected(), false,
                        offline = connection == ConnectivityState.OFFLINE,
                        reconnecting = connection == ConnectivityState.RECONNECTING,
                        projects = workspace.projects.map { DrawerProject(it.id, it.name) })
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
                    val rows = snapshot.rows.map { row ->
                        DrawerRow(DrawerKey(DrawerOrigin.SAVED, row.id), row.title, row.subtitle, row.group,
                            row.agentName, row.indicator?.let(::status), row.unseen, row.pinned, row.archived,
                            if (row.canFile) setOf(DrawerAction.PIN, DrawerAction.ARCHIVE, DrawerAction.RENAME) else emptySet(),
                            row.moveEarlier?.let(::savedMove), row.moveLater?.let(::savedMove))
                    } + snapshot.sessions.map { row ->
                        DrawerRow(DrawerKey(DrawerOrigin.INDEXED, row.key), row.title, row.subtitle,
                            "Computer chats", row.agentName, row.status, row.unseen, false, false, emptySet())
                    }
                    DrawerFrame(rows, selected(), snapshot.hasMore, loading = state is SavedLoadState.Loading,
                        offline = state is SavedLoadState.Failed && state.stale,
                        error = (state as? SavedLoadState.Failed)?.message,
                        projects = snapshot.projects.map { DrawerProject(it.id, it.name) },
                        hasMoreProjects = snapshot.moreProjects)
                }
            }
        }
    }

    override suspend fun send(intent: DrawerIntent): Unit = withContext(Dispatchers.IO) {
        access.withLock {
            when (intent) {
                is DrawerIntent.Open -> when (intent.key.origin) {
                    DrawerOrigin.LIVE -> {
                        val handle = live.openSession(intent.key.id)
                        withContext(Dispatchers.Main) { openLive(handle) }
                        live.markSeen(intent.key.id)
                    }
                    DrawerOrigin.INDEXED -> {
                        val detail = saved.indexedSession(intent.key.id)
                        withContext(Dispatchers.Main) { openIndexed(detail) }
                        detail.row.activityRevision?.let { saved.markIndexedSeen(detail.row.key, it) }
                    }
                    DrawerOrigin.SAVED -> {
                        saved.action(SavedHistoryAction.Open(intent.key.id))
                        withContext(Dispatchers.Main) { openSaved(intent.key.id) }
                    }
                }
                is DrawerIntent.Pin -> { require(intent.key.origin != DrawerOrigin.INDEXED); if (intent.key.origin == DrawerOrigin.LIVE) {
                    if (intent.pinned) live.pinSession(intent.key.id) else live.unpinSession(intent.key.id)
                } else saved.fileChat(intent.key.id, SavedFilingChange.Pin(intent.pinned)) }
                is DrawerIntent.Archive -> { require(intent.key.origin != DrawerOrigin.INDEXED); if (intent.key.origin == DrawerOrigin.LIVE) {
                    if (intent.archived) live.archiveSession(intent.key.id) else live.unarchiveSession(intent.key.id)
                } else saved.fileChat(intent.key.id, SavedFilingChange.Archive(intent.archived)) }
                is DrawerIntent.Rename -> { require(intent.key.origin != DrawerOrigin.INDEXED); if (intent.key.origin == DrawerOrigin.LIVE) {
                    live.renameSession(intent.key.id, intent.title)
                } else saved.fileChat(intent.key.id, SavedFilingChange.Rename(intent.title)) }
                is DrawerIntent.MovePin -> {
                    require(intent.key.origin != DrawerOrigin.INDEXED)
                    require(intent.after == null || intent.after.origin == intent.key.origin)
                    require(intent.before == null || intent.before.origin == intent.key.origin)
                    if (intent.key.origin == DrawerOrigin.LIVE) {
                        live.movePin(intent.key.id, intent.after?.id, intent.before?.id)
                    } else {
                        saved.movePin(intent.key.id, intent.after?.id, intent.before?.id, requireNotNull(intent.revision))
                    }
                }
                DrawerIntent.MoreProjects -> if (activeTab == DrawerTab.SAVED) saved.action(SavedHistoryAction.MoreProjects)
                DrawerIntent.More -> if (activeTab == DrawerTab.SAVED) saved.action(SavedHistoryAction.MoreChats)
                DrawerIntent.Refresh -> if (activeTab == DrawerTab.SAVED) saved.action(SavedHistoryAction.RefreshChats)
            }
        }
    }

    override suspend fun restore(key: DrawerKey) = send(DrawerIntent.Open(key))

    private fun savedMove(move: SavedPinMove) = DrawerPinMove(
        move.after?.let { DrawerKey(DrawerOrigin.SAVED, it) },
        move.before?.let { DrawerKey(DrawerOrigin.SAVED, it) }, move.revision,
    )

    private fun liveRow(row: SessionRow, group: String, pinsReady: Boolean, moves: DrawerPinHints?) = DrawerRow(
        DrawerKey(DrawerOrigin.LIVE, row.id), row.title, row.preview, group, row.agentLine,
        status(row.indicator), row.unseen, row.pinned, row.archived,
        buildSet {
            add(DrawerAction.ARCHIVE)
            add(DrawerAction.RENAME)
            if (pinsReady) add(DrawerAction.PIN)
        },
        moves?.earlier?.let { move -> DrawerPinMove(move.after?.let { DrawerKey(DrawerOrigin.LIVE, it) }, move.before?.let { DrawerKey(DrawerOrigin.LIVE, it) }) },
        moves?.later?.let { move -> DrawerPinMove(move.after?.let { DrawerKey(DrawerOrigin.LIVE, it) }, move.before?.let { DrawerKey(DrawerOrigin.LIVE, it) }) },
    )

    private fun status(value: ChatIndicator): String? = when (value) {
        ChatIndicator.WORKING -> "Working"
        ChatIndicator.AWAITING_INPUT -> "Waiting for you"
        ChatIndicator.ERRORED -> "Couldn't finish"
        ChatIndicator.COMPLETED -> "Finished"
        ChatIndicator.IDLE -> null
    }
}
