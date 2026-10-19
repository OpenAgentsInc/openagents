package com.openagents.app

/** Native resources are retired by Rust's account generation, independently of visible views. */
internal class GenerationOwnedResource<T : AutoCloseable> {
    private var held: Pair<ULong, T>? = null
    val value: T? get() = held?.second
    fun install(generation: ULong, resource: T) {
        check(held == null)
        held = generation to resource
    }
    fun remove(expected: T): Boolean {
        if (held?.second !== expected) return false
        held = null
        return true
    }
    fun retireFor(generation: ULong) {
        val current = held ?: return
        if (current.first == generation) return
        // Remove first: terminal callbacks may run synchronously while closing platform resources.
        held = null
        current.second.close()
    }
}

/** Notification permission affects notification presentation, never microphone/call admission. */
internal fun shouldRequestCallNotifications(api: Int, granted: Boolean, requested: Boolean): Boolean =
    api >= 33 && !granted && !requested
