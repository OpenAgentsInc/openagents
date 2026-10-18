package com.openagents.app.drawer

/** Origin is part of a key: a saved copy and a live room can share an ID. */
enum class DrawerOrigin { LIVE, SAVED }
data class DrawerKey(val origin: DrawerOrigin, val id: String)
enum class DrawerTab { LIVE, SAVED }
enum class DrawerAction { PIN, ARCHIVE, RENAME }
data class DrawerRow(
    val key: DrawerKey,
    val title: String,
    val subtitle: String?,
    val group: String,
    val agentLine: String?,
    val status: String?,
    val unread: Boolean?,
    val pinned: Boolean,
    val archived: Boolean,
    val actions: Set<DrawerAction>,
    val moveEarlier: DrawerPinMove? = null,
    val moveLater: DrawerPinMove? = null,
)
data class DrawerPinMove(val after: DrawerKey?, val before: DrawerKey?, val revision: ULong? = null)

data class DrawerRequest(
    val tab: DrawerTab = DrawerTab.LIVE,
    val query: String = "",
    val archived: Boolean = false,
    val projectId: String? = null,
)
data class DrawerProject(val id: String, val name: String)

data class DrawerFrame(
    val rows: List<DrawerRow>,
    val selected: DrawerKey?,
    val hasMore: Boolean,
    val loading: Boolean = false,
    val offline: Boolean = false,
    val reconnecting: Boolean = false,
    val error: String? = null,
    val projects: List<DrawerProject> = emptyList(),
    val hasMoreProjects: Boolean = false,
) {
    init {
        require(rows.size <= 5_000) { "Drawer frame exceeds its view limit" }
        require(rows.map { it.key }.toSet().size == rows.size) { "Duplicate drawer key" }
    }
}
sealed interface DrawerIntent {
    data class Open(val key: DrawerKey) : DrawerIntent
    data class Pin(val key: DrawerKey, val pinned: Boolean) : DrawerIntent
    data class Archive(val key: DrawerKey, val archived: Boolean) : DrawerIntent
    data class Rename(val key: DrawerKey, val title: String) : DrawerIntent
    /** Drag geometry supplies neighbours; only Rust commits their order. */
    data class MovePin(val key: DrawerKey, val after: DrawerKey?, val before: DrawerKey?, val revision: ULong? = null) : DrawerIntent
    data object MoreProjects : DrawerIntent
    data object More : DrawerIntent
    data object Refresh : DrawerIntent
}

/** The production implementation delegates reads and intents to the Rust core.
 * Implementations must fence account changes. A fixture may return recorded
 * frames, but must not implement search or mutate a local session list.
 */
interface DrawerCore {
    suspend fun load(request: DrawerRequest): DrawerFrame
    suspend fun send(intent: DrawerIntent)
    /** Resolve an existing key. Never create a session while restoring. */
    suspend fun restore(key: DrawerKey)
}
