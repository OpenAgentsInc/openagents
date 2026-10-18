package com.openagents.app.accessibility

import android.os.Build
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityManager
import android.content.Context
import android.widget.EditText
import org.json.JSONObject
import uniffi.openagents_core.CoreAccessibilitySurface

/** Uses the same Rust-owned screen scope and account as the application view. */
interface SemanticsPort : AutoCloseable {
    fun prepare(view: ByteArray): String
    fun acknowledge(instance: String, revision: Long): String?
    fun focused(instance: String, revision: Long, key: String): Boolean
    fun bookmark(): String?
    fun restore(bookmark: String): Boolean
    fun environment(fontScale: Float, density: Float, reducedMotion: Boolean): String
    fun lifecycle(event: String): String
}
class CoreSemanticsPort(private val core: CoreAccessibilitySurface) : SemanticsPort {
    override fun prepare(view: ByteArray) = core.prepare(view)
    override fun acknowledge(instance: String, revision: Long) = core.acknowledge(instance, revision.toULong())
    override fun focused(instance: String, revision: Long, key: String) = core.focused(instance, revision.toULong(), key)
    override fun bookmark() = core.bookmark()
    override fun restore(bookmark: String) = core.restore(bookmark)
    override fun environment(fontScale: Float, density: Float, reducedMotion: Boolean) = core.environment(fontScale, density, reducedMotion)
    override fun lifecycle(event: String) = core.lifecycle(event)
    override fun close() { core.close(); core.destroy() }
}

/** Maps Rust roles to platform nodes. It never dispatches intents or saves field values. */
class NativeSemantics(private val port: SemanticsPort) : AutoCloseable {
    private var generation = 0L
    private var mounted: JSONObject? = null
    private var views: Map<String, View> = emptyMap()
    private var hadKeyboardFocus = false
    private var closed = false

    fun prepare(bytes: ByteArray): JSONObject {
        check(!closed)
        mounted?.let { old ->
            views.entries.firstOrNull { it.value.hasFocus() }?.let { (key, _) ->
                hadKeyboardFocus = true
                port.focused(old.getString("instance"), old.getLong("revision"), key)
            }
        }
        val tree = JSONObject(port.prepare(bytes))
        require(tree.getString("schema") == "rust-native.semantics.v1")
        require(tree.getJSONArray("nodes").length() <= 1024)
        generation++
        return tree
    }
    /** Call only after the view owner acknowledges the successful native mount. */
    fun mounted(tree: JSONObject, controls: Map<String, View>) {
        check(!closed)
        generation++
        mounted = tree
        views = controls.toMap()
        val stamp = generation
        val instance = tree.getString("instance")
        val revision = tree.getLong("revision")
        val nodes = tree.getJSONArray("nodes")
        for (i in 0 until nodes.length()) {
            val node = nodes.getJSONObject(i)
            val key = node.getString("key")
            val view = controls[key] ?: continue
            if (view.id == View.NO_ID) view.id = View.generateViewId()
            val exposed = !node.getBoolean("hidden") && node.getBoolean("focusable")
            view.importantForAccessibility = if (exposed) View.IMPORTANT_FOR_ACCESSIBILITY_YES else View.IMPORTANT_FOR_ACCESSIBILITY_NO
            // Native editors keep their own editable text/selection. Password values never enter semantics.
            if (view !is EditText) view.contentDescription = node.getString("name").ifEmpty { null }
            view.accessibilityLiveRegion = if (node.getBoolean("live")) View.ACCESSIBILITY_LIVE_REGION_POLITE else View.ACCESSIBILITY_LIVE_REGION_NONE
            view.setAccessibilityDelegate(object : View.AccessibilityDelegate() {
                override fun onInitializeAccessibilityNodeInfo(host: View, info: AccessibilityNodeInfo) {
                    super.onInitializeAccessibilityNodeInfo(host, info)
                    if (closed || generation != stamp) return
                    val role = node.getString("role")
                    info.className = when (role) {
                        "button" -> "android.widget.Button"
                        "checkbox" -> "android.widget.CheckBox"
                        "radio" -> "android.widget.RadioButton"
                        "field", "composer" -> "android.widget.EditText"
                        "progress" -> "android.widget.ProgressBar"
                        "image" -> "android.widget.ImageView"
                        else -> info.className
                    }
                    info.isEnabled = node.getBoolean("enabled")
                    info.isPassword = node.getBoolean("password")
                    if (!node.isNull("checked")) { info.isCheckable = true; info.isChecked = node.getBoolean("checked") }
                    if (!node.isNull("selected")) info.isSelected = node.getBoolean("selected")
                    if (Build.VERSION.SDK_INT >= 28) info.isHeading = role == "heading"
                    else if (role == "heading") {
                        info.contentDescription = "Heading. ${node.getString("name")}"
                    }
                    if (Build.VERSION.SDK_INT >= 30) info.stateDescription = node.optString("state").takeUnless { node.isNull("state") }
                    else if (!node.isNull("state") && view !is EditText) info.contentDescription = listOf(node.getString("name"), node.getString("state")).filter { it.isNotEmpty() }.joinToString(", ")
                }
                override fun sendAccessibilityEvent(host: View, eventType: Int) {
                    if (!closed && generation == stamp && eventType in setOf(AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED, AccessibilityEvent.TYPE_VIEW_FOCUSED)) {
                        runCatching { port.focused(instance, revision, key) }
                    }
                    super.sendAccessibilityEvent(host, eventType)
                }
            })
        }
        val order = tree.getJSONArray("order")
        controls.values.forEach { view ->
            view.accessibilityTraversalAfter = View.NO_ID
            view.accessibilityTraversalBefore = View.NO_ID
            view.nextFocusForwardId = View.NO_ID
        }
        val targets = (0 until order.length()).mapNotNull { controls[order.getString(it)] }
        targets.forEachIndexed { i, view ->
            view.accessibilityTraversalAfter = targets.getOrNull(i - 1)?.id ?: View.NO_ID
            view.accessibilityTraversalBefore = targets.getOrNull(i + 1)?.id ?: View.NO_ID
            view.nextFocusForwardId = targets.getOrNull(i + 1)?.id ?: View.NO_ID
            view.nextFocusUpId = targets.getOrNull(i - 1)?.id ?: View.NO_ID
            view.nextFocusDownId = targets.getOrNull(i + 1)?.id ?: View.NO_ID
        }
        val key = port.acknowledge(instance, revision)
        val restoreKeyboardFocus = hadKeyboardFocus
        hadKeyboardFocus = false
        val target = key?.let { controls[it] } ?: return
        // A queued restore must not touch a replacement view or frame.
        target.post {
            if (!closed && generation == stamp && views[key] === target && target.isAttachedToWindow) {
                if (restoreKeyboardFocus && !target.hasFocus()) target.requestFocus()
                val manager = target.context.getSystemService(Context.ACCESSIBILITY_SERVICE) as AccessibilityManager
                if (manager.isEnabled && manager.isTouchExplorationEnabled && !target.isAccessibilityFocused) target.performAccessibilityAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null)
            }
        }
    }
    fun environment(fontScale: Float, density: Float, reducedMotion: Boolean): JSONObject {
        check(!closed)
        return JSONObject(port.environment(fontScale, density, reducedMotion))
    }
    fun bookmark(): String? = if (closed) null else port.bookmark()
    fun restore(bookmark: String): Boolean = !closed && port.restore(bookmark)
    fun lifecycle(event: String): JSONObject {
        check(!closed)
        val effects = JSONObject(port.lifecycle(event))
        if (effects.getBoolean("retire_callbacks")) { generation++; mounted = null }
        return effects
    }
    override fun close() {
        if (closed) return
        closed = true; generation++; mounted = null
        views.values.forEach { it.setAccessibilityDelegate(null) }
        views = emptyMap(); port.close()
    }
}
