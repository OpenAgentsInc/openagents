// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

import uniffi.openagents_core.RevealSlice
import uniffi.openagents_core.RowDisplay
import uniffi.openagents_core.TextRun

/** Converts Rust's age sample and run offset into paint alpha using local elapsed time. */
fun transcriptRevealAlpha(
    rowAgeAtSampleMs: UInt?,
    runStartMs: UInt?,
    elapsedSinceSampleMs: Long,
    durationMs: UInt,
): Float {
    if (rowAgeAtSampleMs == null || runStartMs == null || durationMs == 0u) return 1f
    val age = rowAgeAtSampleMs.toLong() + elapsedSinceSampleMs.coerceAtLeast(0) - runStartMs.toLong()
    return (age.toFloat() / durationMs.toFloat()).coerceIn(0f, 1f)
}

/** A full shaped run uses its most visible slice; bad coverage stays visible. */
internal fun transcriptRunRevealAlpha(
    row: RowDisplay,
    runIndex: UInt,
    run: TextRun,
    elapsedSinceSampleMs: Long,
    durationMs: UInt,
): Float {
    val slices = row.revealSlices.filter { it.runIndex == runIndex }
    if (slices.isEmpty()) {
        return if (row.revealSlices.isEmpty()) {
            transcriptRevealAlpha(row.revealAgeMs, run.revealMs, elapsedSinceSampleMs, durationMs)
        } else 1f
    }

    val runStart = run.start.toLong()
    val runEnd = runStart + run.len.toLong()
    if (runEnd > row.text.length) return 1f
    var cursor = runStart
    var maximum = 0f
    for (slice in slices) {
        if (!validRevealSlice(row, slice, cursor, runStart, runEnd)) return 1f
        cursor += slice.len.toLong()
        maximum = maxOf(
            maximum,
            transcriptRevealAlpha(row.revealAgeMs, slice.revealMs, elapsedSinceSampleMs, durationMs),
        )
    }
    return if (cursor == runEnd) maximum.coerceIn(0f, 1f) else 1f
}

private fun validRevealSlice(row: RowDisplay, slice: RevealSlice, cursor: Long, runStart: Long, runEnd: Long): Boolean {
    val start = slice.start.toLong()
    val end = start + slice.len.toLong()
    if (slice.len == 0u || start != cursor || start < runStart || end <= start || end > runEnd || end > row.text.length) return false
    val from = start.toInt()
    val to = end.toInt()
    return (from == 0 || !row.text[from].isLowSurrogate()) &&
        (to == row.text.length || !row.text[to].isLowSurrogate())
}
