package com.openagents.coder

import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import java.util.concurrent.Executors

internal fun json(vararg values: Pair<String, Any?>) = JSONObject().apply {
    values.forEach { (key, value) -> put(key, value ?: JSONObject.NULL) }
}
internal fun JSONObject.textOrNull(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }
internal fun packet(text: String, schema: String): JSONObject {
    require(text.toByteArray().size in 1..1_048_576) { "Rust returned an invalid or oversized view packet." }
    return JSONObject(text).also {
        require(it.getString("schema") == schema) { "This app does not support the returned view version." }
        for (key in listOf("view", "computers")) it.optJSONObject(key)?.let { view ->
            require(view.getString("schema") == "rust-native.view.v2") { "Unsupported Rust Native view version." }
        }
    }
}

/** One worker owns the Rust reader for its entire lifetime. */
class ReaderBridge(private val storage: DeviceStorage, synthetic: Boolean,
                   loopbackTest: Boolean, private val changed: () -> Unit) {
    companion object {
        // One process-wide owner prevents a retired Activity's refresh from
        // writing a cache after a replacement reader disconnects and erases it.
        private val worker = Executors.newSingleThreadExecutor()
    }
    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L
    private var disposed = false
    private var wantedForeground = false
    private var pendingForeground: Boolean? = null
    private var pendingLifecycle: Boolean? = null
    private var pendingFollow: JSONObject? = null
    var snapshot: JSONObject? = null; private set
    var error: String? = null; private set
    var busy = true; private set

    init {
        worker.execute {
            val result = runCatching {
                handle = CoderNative.createReader(json("cache_dir" to storage.cacheDirectory().path,
                    "secret_hex" to storage.identity("reader"), "synthetic" to synthetic,
                    "loopback_test" to loopbackTest).toString())
                check(handle != 0L) { "The reader could not open its protected local state." }
                call(json("op" to "snapshot"))
            }
            main.post { receive(result) }
        }
    }

    fun request(input: JSONObject, completed: ((Boolean) -> Unit)? = null) {
        if (disposed) return
        if (busy) { error = "The reader is updating. Try again after the refresh finishes."; changed(); completed?.invoke(false); return }
        busy = true; error = null; changed()
        worker.execute {
            val result = runCatching { call(input) }
            main.post { if (!disposed) { receive(result); completed?.invoke(result.getOrNull()?.let {
                it.optBoolean("paired") && it.textOrNull("error") == null } == true) } }
        }
    }

    fun foreground(active: Boolean) {
        wantedForeground = active
        if (busy) pendingForeground = active else request(json("op" to "foreground", "active" to active))
    }
    /** The activity resumed or paused. Rust passes it to every host supervisor. */
    fun lifecycle(active: Boolean) {
        if (busy) pendingLifecycle = active else request(json("op" to "lifecycle", "active" to active))
    }
    /** Poll the Computers surface while it shows; skipped while busy. */
    fun pollComputers() {
        if (!busy) request(json("op" to "computers_refresh"))
    }
    fun refresh(force: Boolean = false) {
        if (wantedForeground && !busy) request(json("op" to if (force) "refresh_now" else "refresh"))
    }
    fun follow(enabled: Boolean, page: String) {
        val value = json("op" to "follow", "enabled" to enabled, "page" to page)
        if (busy) pendingFollow = value else request(value)
    }
    fun dispose() {
        disposed = true
        worker.execute { if (handle != 0L) { CoderNative.destroyReader(handle); handle = 0 } }
    }
    private fun call(value: JSONObject): JSONObject {
        check(handle != 0L) { "The reader has not opened its protected local state." }
        val encoded = value.toString()
        require(encoded.toByteArray().size <= 131_072) { "The native request is too large." }
        return packet(CoderNative.readerCall(handle, encoded), "coder.mobile.v1")
    }
    private fun receive(result: Result<JSONObject>) {
        if (disposed) return
        busy = false
        result.fold({ snapshot = it; error = null }, { error = it.message ?: "The reader could not update." })
        changed()
        pendingForeground?.let { pendingForeground = null; foreground(it); return }
        pendingLifecycle?.let { pendingLifecycle = null; lifecycle(it); return }
        pendingFollow?.let { pendingFollow = null; request(it) }
    }
}
