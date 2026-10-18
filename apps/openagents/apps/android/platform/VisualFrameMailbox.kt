// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

/** Worker delivery only. A retired drain must never clear a replacement's queue. */
internal class VisualFrameMailbox<T>(
    private val limit: Int,
    private val post: (() -> Unit) -> Unit,
    private val receive: (Long, T) -> Unit,
    private val maxWeight: Long = Long.MAX_VALUE,
    private val weight: (T) -> Long = { 1L },
) {
    private val lock = Any()
    private var generation = 0L
    private var scheduled: Long? = null
    private val pending = LinkedHashMap<ULong, T>()

    init { require(limit > 0) }
    fun current(): Long = synchronized(lock) { generation }
    fun isCurrent(value: Long): Boolean = synchronized(lock) { generation == value }
    fun reset(): Long = synchronized(lock) {
        generation++
        pending.clear()
        scheduled = null
        generation
    }
    fun offer(epoch: Long, id: ULong, value: T) {
        val schedule = synchronized(lock) {
            if (epoch != generation) return
            val size = weight(value)
            if (size < 0 || size > maxWeight) return
            pending.remove(id)
            while (pending.isNotEmpty() && (pending.size >= limit || pending.values.sumOf(weight) > maxWeight - size)) {
                pending.remove(pending.keys.first())
            }
            pending[id] = value
            if (scheduled == epoch) false else { scheduled = epoch; true }
        }
        if (schedule) post { drain(epoch) }
    }
    private fun drain(epoch: Long) {
        val batch = synchronized(lock) {
            // In particular, leave the new generation's scheduled flag alone.
            if (epoch != generation) return
            pending.values.toList().also { pending.clear(); scheduled = null }
        }
        for (frame in batch) {
            if (!isCurrent(epoch)) return
            receive(epoch, frame)
        }
    }
}

/** Keep Rust's vector/text fallback if bitmap allocation or pixel copy fails. */
internal fun <T> visualRasterOrFallback(
    allocate: () -> T,
    copy: (T) -> Unit,
    dispose: (T) -> Unit,
): T? {
    var raster: T? = null
    return try {
        allocate().also { raster = it; copy(it) }
    } catch (_: RuntimeException) {
        raster?.let(dispose)
        null
    } catch (_: OutOfMemoryError) {
        raster?.let(dispose)
        null
    }
}
