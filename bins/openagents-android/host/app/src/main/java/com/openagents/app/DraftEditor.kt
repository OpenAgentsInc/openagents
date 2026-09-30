package com.openagents.app

import org.json.JSONArray
import org.json.JSONObject

/** Rust Native's shared composer editor (`rust_native::edit::mirror`), over JNI. */
object EditorNative {
    init { System.loadLibrary("openagents_mobile") }
    @JvmStatic external fun create(): Long
    @JvmStatic external fun call(handle: Long, request: String): String
    @JvmStatic external fun destroy(handle: Long)
}

/**
 * One composer field's draft, edited through the shared editor: the field
 * reports each change, and the state Rust returns is what it shows.
 * Deletion never splits a grapheme, an IME composition is one undo step,
 * undo and redo are the shared history's, and a stamp from an older draft
 * is refused. Positions are UTF-16, as Android's text is.
 */
class DraftEditor(private val native: Native = Jni) {
    /** The JNI calls, replaceable in unit tests. */
    interface Native {
        fun create(): Long
        fun call(handle: Long, request: String): String
        fun destroy(handle: Long)
    }

    object Jni : Native {
        override fun create() = EditorNative.create()
        override fun call(handle: Long, request: String) = EditorNative.call(handle, request)
        override fun destroy(handle: Long) = EditorNative.destroy(handle)
    }

    data class State(
        val token: String,
        val lifetime: Long,
        val revision: Long,
        val text: String,
        val anchor: Int,
        val caret: Int,
        val marked: Pair<Int, Int>?,
        val canUndo: Boolean,
        val canRedo: Boolean,
    ) {
        val start get() = minOf(anchor, caret)
        val end get() = maxOf(anchor, caret)
    }

    private var handle = runCatching { native.create() }.getOrDefault(0L)
    var state: State? = null
        private set

    /** Whether the shared editor is available; without it the field edits on its own. */
    val live get() = handle != 0L

    /** Mounts the view's composer; true when a new token replaced the draft. */
    fun mount(token: String, maxBytes: Int, draft: String?): Boolean {
        val request = JSONObject().put("op", "mount").put("token", token).put("max_bytes", maxBytes)
        if (draft != null) request.put("draft", draft)
        val reply = call(request) ?: return false
        return reply.optBoolean("replaced")
    }

    /** Applies one change; the reply's state is the draft to show, also after a refusal. */
    fun apply(change: JSONObject): State? {
        val current = state ?: return null
        val stamp = JSONObject().put("token", current.token).put("lifetime", current.lifetime)
            .put("revision", current.revision)
        call(JSONObject().put("op", "apply").put("stamp", stamp).put("change", change))
        return state
    }

    fun sync(text: String, start: Int, end: Int, marked: Pair<Int, Int>?) = apply(JSONObject()
        .put("op", "sync").put("text", text).put("selection", JSONArray(listOf(start, end)))
        .put("marked", marked?.let { JSONArray(listOf(it.first, it.second)) } ?: JSONObject.NULL)
        .put("at_ms", now()))

    fun select(start: Int, end: Int) =
        apply(JSONObject().put("op", "select").put("selection", JSONArray(listOf(start, end))))

    fun delete(backwards: Boolean) =
        apply(JSONObject().put("op", "delete").put("backwards", backwards).put("at_ms", now()))

    fun undo() = apply(JSONObject().put("op", "undo"))
    fun redo() = apply(JSONObject().put("op", "redo"))

    fun close() {
        if (handle != 0L) runCatching { native.destroy(handle) }
        handle = 0L
    }

    private fun call(request: JSONObject): JSONObject? {
        if (handle == 0L) return null
        val reply = runCatching { JSONObject(native.call(handle, request.toString())) }.getOrNull() ?: return null
        reply.optJSONObject("state")?.let { state = parse(it) }
        return reply
    }

    companion object {
        /** Milliseconds on a monotonic clock, for undo coalescing. */
        fun now(): Long = System.nanoTime() / 1_000_000

        fun parse(value: JSONObject): State {
            val stamp = value.getJSONObject("stamp")
            val selection = value.getJSONArray("selection")
            val marked = value.optJSONArray("marked")
            return State(
                token = stamp.getString("token"),
                lifetime = stamp.getLong("lifetime"),
                revision = stamp.getLong("revision"),
                text = value.getString("text"),
                anchor = selection.getInt(0),
                caret = selection.getInt(1),
                marked = marked?.let { it.getInt(0) to it.getInt(1) },
                canUndo = value.optBoolean("can_undo"),
                canRedo = value.optBoolean("can_redo"),
            )
        }
    }
}
