package com.openagents.app

/** One UI callback whose retired owner cannot remove its replacement. Main-thread only. */
internal class ForegroundCallback<T> {
    private var current: ((T) -> Unit)? = null
    fun install(callback: (T) -> Unit): AutoCloseable {
        current = callback
        return AutoCloseable { if (current === callback) current = null }
    }
    fun deliver(value: T): Boolean {
        val callback = current ?: return false
        callback(value)
        return true
    }
    fun clear() { current = null }
}
