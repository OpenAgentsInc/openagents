// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.view.View
import android.view.ViewGroup
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.CoreLabCardInterface
import uniffi.openagents_core.LabCardEffect
import uniffi.openagents_core.LabCardProjection
import uniffi.openagents_core.LabSurfaceRegistration

/** The Rust owner resolves this identity; Kotlin never interprets a Lab intent. */
data class RustNativeCardActivation(val instance: String, val revision: Long, val node: String)

interface RustNativeLabMount : AutoCloseable {
    fun mount(json: ByteArray)
    override fun close()
}
fun interface RustNativeLabMountFactory {
    fun create(
        surfaces: List<LabSurfaceRegistration>,
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (String, Long, String, String) -> Unit,
        onDisplayed: (String, Long) -> Unit,
    ): RustNativeLabMount
}

/** A native media view owns its worker/player/bitmap handles, including on failure. */
interface OwnedLabSurface : AutoCloseable {
    val view: View
    override fun close()
}
fun interface LabSurfaceRenderer {
    // Fetch only through the CoreLabCard that issued this registration.
    fun open(surface: LabSurfaceRegistration): OwnedLabSurface
}

/** One UI-thread surface lifetime, with independently fenced worker callbacks. */
class RustNativeLabCardBridge(
    private val postToMain: (() -> Unit) -> Unit,
    private val mountFactory: RustNativeLabMountFactory,
) : AutoCloseable {
    constructor(context: Context, container: ViewGroup, renderer: LabSurfaceRenderer, postToMain: (() -> Unit) -> Unit) :
        this(postToMain, SemanticLabMountFactory(context, container, renderer))
    private val generation = java.util.concurrent.atomic.AtomicLong(0)
    private var mount: RustNativeLabMount? = null
    private var releaseCore: (() -> Unit)? = null
    private var boundCore: CoreLabCardInterface? = null
    private var closed = false

    /** The only production entry: a Rust-validated, account-fenced projection. */
    fun show(core: CoreLabCardInterface, onEffect: (LabCardEffect) -> Unit) {
        check(!closed)
        if (boundCore === core) return
        val projection = try { core.snapshot() } catch (error: Exception) { disposeCore(core); throw error }
        try {
            show(projection,
                onActivation = { event -> onEffect(core.activate(event.instance, event.revision.toULong(), event.node)) },
                onDisplayed = { instance, revision -> core.acknowledge(instance, revision.toULong()) },
            )
            boundCore = core
            releaseCore = { disposeCore(core) }
        } catch (error: Throwable) {
            disposeCore(core)
            throw error
        }
    }

    /** Fixture seam. Documents and registrations must come from the same Rust owner. */
    internal fun show(
        projection: LabCardProjection,
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (String, Long, String, String) -> Unit = { _, _, _, _ -> },
        onDisplayed: (String, Long) -> Unit = { _, _ -> },
    ) {
        check(!closed)
        require(projection.json.isNotEmpty() && projection.json.size <= 512 * 1024)
        require(projection.surfaces.size <= 8 && projection.surfaces.map { it.resource }.toSet().size == projection.surfaces.size)
        clear()
        val epoch = generation.get()
        fun deliver(action: () -> Unit) = postToMain {
            if (!closed && generation.get() == epoch && mount != null) action()
        }
        val candidate = mountFactory.create(projection.surfaces,
            onActivation = { event -> deliver { onActivation(event) } },
            onFieldChanged = { instance, revision, key, value -> deliver { onFieldChanged(instance, revision, key, value) } },
            onDisplayed = { instance, revision -> deliver { onDisplayed(instance, revision) } },
        )
        mount = candidate
        try { candidate.mount(projection.json) } catch (error: Throwable) {
            // Retain and dispose the handle even when mounting never returns.
            try { clear() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
            throw error
        }
    }
    private fun disposeCore(core: CoreLabCardInterface) {
        try { core.dispose() } finally { (core as? AutoCloseable)?.close() }
    }
    fun clear() {
        generation.incrementAndGet()
        val previous = mount
        val core = releaseCore
        mount = null
        releaseCore = null
        boundCore = null
        try { previous?.close() } finally { core?.invoke() }
    }
    override fun close() {
        if (closed) return
        closed = true
        clear()
    }
}

private class SemanticLabMountFactory(
    private val context: Context,
    private val container: ViewGroup,
    private val renderer: LabSurfaceRenderer,
) : RustNativeLabMountFactory {
    override fun create(
        surfaces: List<LabSurfaceRegistration>,
        onActivation: (RustNativeCardActivation) -> Unit,
        onFieldChanged: (String, Long, String, String) -> Unit,
        onDisplayed: (String, Long) -> Unit,
    ): RustNativeLabMount = SemanticLabMount(context, container, surfaces, renderer, onActivation, onFieldChanged, onDisplayed)
}
private class SemanticLabMount(
    context: Context,
    private val container: ViewGroup,
    surfaces: List<LabSurfaceRegistration>,
    renderer: LabSurfaceRenderer,
    onActivation: (RustNativeCardActivation) -> Unit,
    onFieldChanged: (String, Long, String, String) -> Unit,
    onDisplayed: (String, Long) -> Unit,
) : RustNativeLabMount {
    private val resources = LabSurfaceResources(surfaces, renderer::open)
    private val owned = java.util.IdentityHashMap<View, String>()
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key -> onActivation(RustNativeCardActivation(instance, revision, key)) },
        changeField = onFieldChanged,
        acknowledge = onDisplayed,
        extensions = mapOf("surface" to object : SemanticViewAdapter.Extension {
            override fun mount(node: org.json.JSONObject, previous: View?): View {
                val resource = node.getJSONObject("element").getJSONObject("props").getString("resource")
                if (previous != null) { require(owned[previous] == resource); return previous }
                val surface = resources.acquire(resource)
                try {
                    val view = surface.view
                    require(!owned.containsKey(view)) { "Lab surfaces must own distinct views" }
                    owned[view] = resource
                    return view
                } catch (error: Throwable) {
                    try { resources.release(resource) } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
                    throw error
                }
            }
            override fun dispose(view: View) { owned.remove(view)?.let(resources::release) }
        }),
    )
    override fun mount(json: ByteArray) = adapter.mount(container, json)
    override fun close() {
        try { adapter.dispose() } finally {
            // Includes resources acquired before a partially mounted node was registered.
            owned.clear()
            try { container.removeAllViews() } finally { resources.close() }
        }
    }
}

/** Transfers one Rust visual handle to one native view. */
class OwnedVisualLabSurface private constructor(private val visual: AndroidVisualView) : OwnedLabSurface {
    override val view: View get() = visual
    override fun close() = visual.release()
    companion object {
        fun create(context: Context, session: uniffi.openagents_core.VisualSession,
            dark: Boolean, reduceMotion: Boolean): OwnedVisualLabSurface {
            var visual: AndroidVisualView? = null
            try {
                val created = AndroidVisualView(context)
                visual = created
                created.bind(session, dark, reduceMotion)
                return OwnedVisualLabSurface(created)
            } catch (error: Throwable) {
                try { visual?.release() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
                try { session.close() } catch (cleanup: Throwable) { error.addSuppressed(cleanup) }
                throw error
            }
        }
    }
}
