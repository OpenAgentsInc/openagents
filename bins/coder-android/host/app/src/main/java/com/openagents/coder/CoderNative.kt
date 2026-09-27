package com.openagents.coder

import android.view.Surface

/** Bounded JSON bridge; Rust owns application state, permissions, and transport. */
object CoderNative {
    init { System.loadLibrary("coder_mobile") }
    @JvmStatic external fun createReader(config: String): Long
    @JvmStatic external fun readerCall(handle: Long, request: String): String
    @JvmStatic external fun destroyReader(handle: Long)
    @JvmStatic external fun verseBlueprint(): String
    @JvmStatic external fun createVerse(surface: Surface, config: String): Long
    @JvmStatic external fun attachVerse(handle: Long, surface: Surface, config: String)
    @JvmStatic external fun detachVerse(handle: Long)
    @JvmStatic external fun verseCall(handle: Long, request: String): String
    @JvmStatic external fun destroyVerse(handle: Long)
}
