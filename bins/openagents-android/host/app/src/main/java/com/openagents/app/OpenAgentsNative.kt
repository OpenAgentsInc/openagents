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
    /** Publishes the JVM and [context] (the application context) to iroh's DNS resolver; call it once, before [create]. */
    @JvmStatic external fun installContext(context: android.content.Context)
    /**
     * Whether this build shows the features still in development (the Verse,
     * the Gym, Trainer, Playtest, and Tailnet): off unless the Rust library
     * was built with `OPENAGENTS_MOBILE_PREVIEW=on`.
     */
    @JvmStatic external fun preview(): Boolean
    @JvmStatic external fun create(config: String): Long
    @JvmStatic external fun call(handle: Long, request: String): String
    @JvmStatic external fun destroy(handle: Long)
    /** Attaches a photo's encoded bytes to the open chat's draft; answers with the app packet. */
    @JvmStatic external fun attachImage(handle: Long, name: String, bytes: ByteArray): String
    /** The encoded bytes the chat's `image:` surface shows, or an empty array. */
    @JvmStatic external fun image(handle: Long, resource: String): ByteArray
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
    /**
     * On the app worker: takes the app's link to the paired computer [host]
     * for Everglade's studio and answers a token for [verseStudioConnect].
     */
    @JvmStatic external fun studioLinks(handle: Long, host: String): Long
    /**
     * On the main thread: connects the world's studio through the link taken
     * under [token]. Answers `{"connected":true,"rights":[...]}` or
     * `{"connected":false,"error":"..."}`.
     */
    @JvmStatic external fun verseStudioConnect(handle: Long, token: Long): String
}
