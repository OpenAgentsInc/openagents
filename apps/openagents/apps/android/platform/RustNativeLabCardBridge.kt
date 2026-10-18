// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

/** A checked activation from the Rust Native view currently mounted by the host. */
data class RustNativeCardActivation(
    val instance: String,
    val revision: ULong,
    val node: String,
)

/** Handle returned by the platform adapter; closing it releases that mount. */
interface RustNativeCardMount : AutoCloseable {
    override fun close()
}

/** Replaceable seam for the shared Rust Native Android Views adapter. */
fun interface RustNativeCardAdapter {
    fun mount(viewJson: String, onActivation: (RustNativeCardActivation) -> Unit): RustNativeCardMount
}

/**
 * Owns one Lab card mount and forwards only the adapter's revision-bound event.
 * The application still resolves that event against its current Rust view.
 */
class RustNativeLabCardBridge(
    private val adapter: RustNativeCardAdapter,
    private val postToMain: (() -> Unit) -> Unit,
) : AutoCloseable {
    private val generation = java.util.concurrent.atomic.AtomicLong(0)
    private var mount: RustNativeCardMount? = null

    fun show(viewJson: String, onActivation: (RustNativeCardActivation) -> Unit) {
        require(viewJson.isNotEmpty() && viewJson.toByteArray(Charsets.UTF_8).size <= MAX_VIEW_BYTES) {
            "Rust Native view exceeds its byte bound"
        }
        clear()
        val mountedGeneration = generation.get()
        mount = adapter.mount(viewJson) { activation ->
            postToMain {
                if (generation.get() == mountedGeneration) onActivation(activation)
            }
        }
    }

    fun clear() {
        generation.incrementAndGet()
        mount?.close()
        mount = null
    }

    override fun close() = clear()

    private companion object {
        const val MAX_VIEW_BYTES = 512 * 1024
    }
}
