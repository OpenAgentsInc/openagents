// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import uniffi.openagents_core.LayoutFrame
import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.RowPlacement
import uniffi.openagents_core.StyleDesc

/** Rust geometry, or a recorded fixture implementing the same read-only contract. */
interface TranscriptFrame {
    val revision: ULong
    val width: Float
    val totalHeight: Float
    fun styles(): List<StyleDesc>
    fun rowsIn(top: Float, bottom: Float): List<RowPlacement>
    fun indexAt(y: Float): UInt?
    fun placement(index: UInt): RowPlacement?
    fun indexOf(key: ULong): UInt?
    fun display(index: UInt): RowDisplay?
}

class RustTranscriptFrame(private val frame: LayoutFrame) : TranscriptFrame, AutoCloseable {
    override val revision get() = frame.revision()
    override val width get() = frame.width()
    override val totalHeight get() = frame.totalHeight()
    override fun styles() = frame.styles()
    override fun rowsIn(top: Float, bottom: Float) = frame.rowsIn(top, bottom)
    override fun indexAt(y: Float) = frame.indexAt(y)
    override fun placement(index: UInt) = frame.placement(index)
    override fun indexOf(key: ULong) = frame.indexOf(key)
    override fun display(index: UInt) = frame.display(index)
    override fun close() = frame.close()
}

/** Viewport state only. Row measurement, ordering, and visibility stay in Rust. */
class TranscriptViewport(private val cacheLimit: Int = 96) {
    init { require(cacheLimit > 0) }
    private data class Anchor(val key: ULong, val inside: Float)
    private data class CacheKey(val key: ULong, val version: ULong, val width: Float)
    private val cache = object : LinkedHashMap<CacheKey, RowDisplay>(16, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<CacheKey, RowDisplay>?) = size > cacheLimit
    }
    var frame: TranscriptFrame? = null
        private set
    var offset = 0f
        private set
    var height = 0f
        private set
    var following = true
        private set
    val cachedRows get() = cache.size
    val maxOffset get() = ((frame?.totalHeight ?: 0f) - height).coerceAtLeast(0f)

    fun resize(height: Float) {
        require(height.isFinite() && height >= 0)
        this.height = height
        offset = if (following) maxOffset else offset.coerceIn(0f, maxOffset)
    }

    fun submit(next: TranscriptFrame) {
        require(next.width.isFinite() && next.width >= 0 && next.totalHeight.isFinite() && next.totalHeight >= 0)
        val old = frame
        val anchor = old?.indexAt(offset)?.let(old::placement)?.let { Anchor(it.key, offset - it.y) }
        // Styles are per-frame; a font change may reuse a style ID and row version.
        if (old?.styles() != next.styles()) cache.clear()
        frame = next
        offset = if (following) maxOffset else {
            val row = anchor?.let { next.indexOf(it.key) }?.let(next::placement)
            val desired = if (row != null && anchor != null) {
                row.y + anchor.inside.coerceIn(0f, (row.height - .001f).coerceAtLeast(0f))
            } else offset
            desired.coerceIn(0f, maxOffset)
        }
    }

    fun userScrollTo(y: Float, dragging: Boolean) {
        require(y.isFinite())
        offset = y.coerceIn(0f, maxOffset)
        // A drag at the tail must release follow before a streaming resize.
        following = !dragging && maxOffset - offset <= 2f
    }

    fun followTail() { following = true; offset = maxOffset }

    fun visibleRows(): List<RowPlacement> = if (height <= 0f) emptyList() else {
        frame?.rowsIn(offset, offset + height).orEmpty()
    }

    fun display(row: RowPlacement): RowDisplay? {
        val current = frame ?: return null
        val key = CacheKey(row.key, row.version, current.width)
        return cache[key] ?: current.display(row.index)?.also {
            require(it.key == row.key && it.version == row.version && it.width == current.width && it.height == row.height)
            cache[key] = it
        }
    }

    fun clear() { cache.clear(); frame = null; offset = 0f; following = true }
}
