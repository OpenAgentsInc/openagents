// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.transcript

/** Rust-selected UTF-16 slices and origins; fallback is synthetic, not device output. */
internal data class RecordedRun(val start: Int, val length: Int, val x: Float, val baseline: Float, val width: Float)
internal data class RecordedTextRow(val text: String, val height: Float, val runs: List<RecordedRun>)
internal val recordedText = listOf(
    RecordedTextRow("Latin ffi AV cafe", 39.0f, listOf(
        RecordedRun(0, 17, 18.0f, 32.35749816894531f, 118.02450561523438f),
    )),
    RecordedTextRow("漢字かな中文", 37.0f, listOf(
        RecordedRun(0, 6, 18.0f, 30.357500076293945f, 108.9000015258789f),
    )),
    RecordedTextRow("🙂 👨‍👩‍👧‍👦", 37.0f, listOf(
        RecordedRun(0, 14, 18.0f, 30.357500076293945f, 149.3249969482422f),
    )),
    RecordedTextRow("é ä", 37.0f, listOf(
        RecordedRun(0, 5, 18.0f, 30.357500076293945f, 22.472999572753906f),
    )),
    RecordedTextRow("العربية שלום", 37.0f, listOf(
        RecordedRun(0, 12, 18.0f, 30.357500076293945f, 203.77499389648438f),
    )),
    RecordedTextRow("x != y -> z", 37.0f, listOf(
        RecordedRun(0, 11, 18.0f, 30.357500076293945f, 76.09800720214844f),
    )),
)
