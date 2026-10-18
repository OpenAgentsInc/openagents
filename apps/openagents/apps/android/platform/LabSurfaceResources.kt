// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import uniffi.openagents_core.LabSurfaceRegistration

/** Registry lifetime matches one Rust projection; unknown and retired ids fail closed. */
internal class LabSurfaceResources<T : AutoCloseable>(
    registrations: List<LabSurfaceRegistration>,
    private val open: (LabSurfaceRegistration) -> T,
) : AutoCloseable {
    private val registered = registrations.associateBy { it.resource }
    private val owned = mutableMapOf<String, T>()
    private var closed = false
    init {
        require(registrations.size <= 8 && registered.size == registrations.size)
        require(registered.keys.all { it.isNotEmpty() && it.length <= 256 })
    }
    fun acquire(resource: String): T {
        check(!closed)
        val registration = requireNotNull(registered[resource]) { "Unregistered Lab surface" }
        return owned.getOrPut(resource) { open(registration) }
    }
    fun release(resource: String) { owned.remove(resource)?.close() }
    override fun close() {
        if (closed) return
        closed = true
        val remaining = owned.values.toList()
        owned.clear()
        var failure: Throwable? = null
        for (resource in remaining) try { resource.close() } catch (error: Throwable) {
            if (failure == null) failure = error else failure.addSuppressed(error)
        }
        failure?.let { throw it }
    }
}
