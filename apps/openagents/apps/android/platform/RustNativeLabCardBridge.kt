// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.view.ViewGroup
import android.widget.TextView
import com.openagents.app.SemanticViewAdapter

/** A checked activation from the Rust Native view currently mounted by the host. */
data class RustNativeCardActivation(
    val instance: String,
    val revision: Long,
    val node: String,
)

/** Small seam for fixture tests; production mounts through SemanticViewAdapter. */
interface RustNativeLabMount : AutoCloseable {
    fun mount(json: ByteArray)
    override fun close()
}

fun interface RustNativeLabMountFactory {
    fun create(
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (instance: String, revision: Long, key: String, value: String) -> Unit,
        onDisplayed: (instance: String, revision: Long) -> Unit,
    ): RustNativeLabMount
}

/** Mounts bounded Rust Lab views and ignores events from replaced revisions. */
class RustNativeLabCardBridge(
    private val postToMain: (() -> Unit) -> Unit,
    private val mountFactory: RustNativeLabMountFactory,
) : AutoCloseable {
    constructor(context: Context, container: ViewGroup, postToMain: (() -> Unit) -> Unit) :
        this(postToMain, SemanticLabMountFactory(context, container))
    private val generation = java.util.concurrent.atomic.AtomicLong(0)
    private var mount: RustNativeLabMount? = null

    fun show(
        viewJson: String,
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (instance: String, revision: Long, key: String, value: String) -> Unit = { _, _, _, _ -> },
        onDisplayed: (instance: String, revision: Long) -> Unit = { _, _ -> },
    ) {
        val json = viewJson.toByteArray(Charsets.UTF_8)
        require(json.isNotEmpty() && json.size <= MAX_VIEW_BYTES) {
            "Rust Native view exceeds its byte bound"
        }
        clear()
        val mountedGeneration = generation.get()
        mount = mountFactory.create(
            onActivation = { event ->
                postToMain {
                    if (generation.get() == mountedGeneration) onActivation(event)
                }
            },
            onFieldChanged = { instance, revision, key, value ->
                postToMain {
                    if (generation.get() == mountedGeneration) onFieldChanged(instance, revision, key, value)
                }
            },
            onDisplayed = { instance, revision ->
                postToMain {
                    if (generation.get() == mountedGeneration) onDisplayed(instance, revision)
                }
            },
        ).also { it.mount(json) }
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

private class SemanticLabMountFactory(
    private val context: Context,
    private val container: ViewGroup,
) : RustNativeLabMountFactory {
    override fun create(
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (String, Long, String, String) -> Unit,
        onDisplayed: (String, Long) -> Unit,
    ): RustNativeLabMount = SemanticLabMount(context, container, onActivation, onFieldChanged, onDisplayed)
}

private class SemanticLabMount(
    context: Context,
    private val container: ViewGroup,
    onActivation: (RustNativeCardActivation) -> Unit,
    onFieldChanged: (String, Long, String, String) -> Unit,
    onDisplayed: (String, Long) -> Unit,
) : RustNativeLabMount {
    private val adapter = SemanticViewAdapter(
        context = context,
        activate = { instance, revision, key -> onActivation(RustNativeCardActivation(instance, revision, key)) },
        changeField = onFieldChanged,
        acknowledge = onDisplayed,
        extensions = mapOf(
            "surface" to object : SemanticViewAdapter.Extension {
                override fun mount(node: org.json.JSONObject, previous: android.view.View?): android.view.View {
                    val view = previous as? TextView ?: TextView(context)
                    view.text = node.getJSONObject("element").getJSONObject("props").getString("label")
                    view.contentDescription = view.text
                    return view
                }

                override fun dispose(view: android.view.View) {
                    view.setOnClickListener(null)
                }
            },
        ),
    )

    override fun mount(json: ByteArray) = adapter.mount(container, json)
    override fun close() = adapter.dispose()
}
