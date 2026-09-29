package com.openagents.app

import android.view.Surface

/**
 * The JNI surface of `openagents-mobile` (`crates/openagents-mobile/src/android`).
 * Requests and packets are the same JSON the iOS host sends through the C ABI.
 * Call the app functions on one background worker, and the Verse functions on
 * the main thread. A failed call throws a RuntimeException with Rust's reason.
 */
object OpenAgentsNative {
    init { System.loadLibrary("openagents_mobile") }
    @JvmStatic external fun create(config: String): Long
    @JvmStatic external fun call(handle: Long, request: String): String
    @JvmStatic external fun destroy(handle: Long)
    /**
     * Blocks until the app packet changes: Rust's change count once it
     * differs from [seen], or [seen] after [timeoutMs]. Call it on a thread
     * of its own, never the app worker; it takes no handle.
     */
    @JvmStatic external fun waitChange(seen: Long, timeoutMs: Int): Long
    /** Whether the Coder tab shows; while it shows a live chat, [waitChange] returns every second. */
    @JvmStatic external fun coderShown(shown: Boolean)
    @JvmStatic external fun verseCreate(surface: Surface, config: String): Long
    @JvmStatic external fun verseAttach(handle: Long, surface: Surface, config: String)
    @JvmStatic external fun verseDetach(handle: Long)
    @JvmStatic external fun verseCall(handle: Long, request: String): String
    @JvmStatic external fun verseDestroy(handle: Long)
}
