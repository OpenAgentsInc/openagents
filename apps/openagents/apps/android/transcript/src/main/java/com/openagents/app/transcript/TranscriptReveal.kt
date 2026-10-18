// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

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
