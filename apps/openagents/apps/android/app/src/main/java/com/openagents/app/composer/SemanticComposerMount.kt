// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.Context
import android.widget.FrameLayout
import com.openagents.app.SemanticViewAdapter
import java.io.Closeable
import java.nio.charset.StandardCharsets

/** Mounts shared Rust Native question and approval views through the app adapter. */
class SemanticComposerMount(
    private val context: Context,
    private val core: ComposerCorePort,
    private val extensions: Map<String, SemanticViewAdapter.Extension> = emptyMap(),
) : RustNativeComposerMount {
    override fun mount(viewJson: String, target: FrameLayout): Closeable {
        val bytes = viewJson.toByteArray(StandardCharsets.UTF_8)
        require(bytes.isNotEmpty() && bytes.size <= MAX_VIEW_BYTES)
        val adapter = SemanticViewAdapter(
            context = context,
            activate = core::activate,
            changeField = core::changeField,
            acknowledge = core::acknowledge,
            extensions = extensions,
        )
        try {
            adapter.mount(target, bytes)
        } catch (error: Exception) {
            adapter.dispose()
            throw error
        }
        return Closeable { adapter.dispose() }
    }

    private companion object {
        const val MAX_VIEW_BYTES = 512 * 1024
    }
}
