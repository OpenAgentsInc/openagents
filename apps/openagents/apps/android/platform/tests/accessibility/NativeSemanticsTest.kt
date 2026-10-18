package com.openagents.app.accessibility

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.test.InstrumentationTestCase
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.openagents.app.SemanticViewAdapter
import org.json.JSONArray
import org.json.JSONObject

/** Authored synthetic metadata, not captured Android output. No network or account is used. */
class NativeSemanticsTest : InstrumentationTestCase() {
    private fun node(key: String, role: String, name: String, password: Boolean = false) = JSONObject()
        .put("key", key).put("role", role).put("name", name).put("enabled", true)
        .put("selected", if (role == "radio") false else JSONObject.NULL)
        .put("checked", if (role == "radio") false else JSONObject.NULL).put("password", password)
        .put("focusable", true).put("hidden", false).put("live", false).put("state", JSONObject.NULL)
        .put("actions", JSONArray())
    private fun tree(revision: Long) = JSONObject().put("schema", "rust-native.semantics.v1")
        .put("instance", "fixture").put("revision", revision).put("modal", JSONObject.NULL)
        .put("nodes", JSONArray().put(node("heading", "heading", "Settings"))
            .put(node("password", "field", "Password", true)).put(node("save", "button", "Save"))
            .put(node("choice", "radio", "Use dark theme")))
        .put("order", JSONArray().put("heading").put("password").put("save").put("choice"))
    private class FakePort : SemanticsPort {
        var prepared = ""
        var closed = 0
        var bookmark: String? = null
        val focused = mutableListOf<String>()
        val events = mutableListOf<String>()
        override fun prepare(view: ByteArray) = prepared
        override fun acknowledge(instance: String, revision: Long): String? = "save"
        override fun focused(instance: String, revision: Long, key: String): Boolean { focused.add("$instance:$revision:$key"); return true }
        override fun bookmark() = bookmark
        override fun restore(bookmark: String): Boolean { this.bookmark = bookmark; return true }
        override fun environment(fontScale: Float, density: Float, reducedMotion: Boolean) = JSONObject().put("font_scale", fontScale).put("density", density).put("reduced_motion", reducedMotion).toString()
        override fun lifecycle(event: String): String {
            events.add(event)
            return JSONObject().put("retire_callbacks", event in setOf("rotation", "background", "process_recreated", "permission_revoked"))
                .put("revalidate_permissions", event == "permission_revoked").toString()
        }
        override fun close() { closed++ }
    }
    private class TrackingButton(context: Context) : Button(context) {
        var delegate: View.AccessibilityDelegate? = null
        override fun setAccessibilityDelegate(value: View.AccessibilityDelegate?) { delegate = value; super.setAccessibilityDelegate(value) }
    }
    fun testRolesNamesPasswordAndTraversal() = runTestOnUiThread {
        val context = instrumentation.targetContext
        val port = FakePort(); val bridge = NativeSemantics(port)
        val heading = Button(context); val password = EditText(context); val save = TrackingButton(context)
        val choice = android.widget.RadioButton(context)
        bridge.mounted(tree(1), mapOf("heading" to heading, "password" to password, "save" to save, "choice" to choice))
        val info = save.createAccessibilityNodeInfo()
        assertEquals("android.widget.Button", info.className.toString())
        assertEquals("Save", info.contentDescription)
        assertTrue(password.createAccessibilityNodeInfo().isPassword)
        assertEquals("android.widget.RadioButton", choice.createAccessibilityNodeInfo().className.toString())
        assertTrue(choice.createAccessibilityNodeInfo().isCheckable)
        assertFalse(choice.createAccessibilityNodeInfo().isChecked)
        assertEquals(password.id, save.accessibilityTraversalAfter)
        assertEquals(save.id, password.nextFocusForwardId)
        val headingInfo = heading.createAccessibilityNodeInfo()
        if (Build.VERSION.SDK_INT >= 28) assertTrue(headingInfo.isHeading)
        else assertEquals("Heading. Settings", headingInfo.contentDescription.toString())
        bridge.close(); assertEquals(1, port.closed)
    }
    fun testReplacedAndClosedDelegatesCannotMoveFocus() = runTestOnUiThread {
        val port = FakePort(); val bridge = NativeSemantics(port)
        val old = TrackingButton(instrumentation.targetContext)
        bridge.mounted(tree(1), mapOf("save" to old))
        val stale = old.delegate!!
        val replacement = TrackingButton(instrumentation.targetContext)
        bridge.mounted(tree(2), mapOf("save" to replacement))
        stale.sendAccessibilityEvent(old, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
        assertTrue(port.focused.isEmpty())
        replacement.delegate!!.sendAccessibilityEvent(replacement, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
        assertEquals(listOf("fixture:2:save"), port.focused)
        val delegate = replacement.delegate!!; bridge.close()
        delegate.sendAccessibilityEvent(replacement, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
        assertEquals(1, port.focused.size)
    }
    fun testRotationProcessRecreationAndPermissionRevocationRetireCallbacks() = runTestOnUiThread {
        val port = FakePort(); val bridge = NativeSemantics(port)
        val button = TrackingButton(instrumentation.targetContext)
        bridge.mounted(tree(1), mapOf("save" to button)); val delegate = button.delegate!!
        val received = mutableListOf<JSONObject>()
        val lifecycle = AccessibilityLifecycle(bridge, { received.add(it) }, { _, _, _ -> })
        port.bookmark = "{\"scope\":\"synthetic-account/settings\",\"key\":\"save\"}"
        val bookmark = lifecycle.saveFocus()!!
        lifecycle.rotation(); lifecycle.processRecreated(); lifecycle.permissionRevoked()
        assertTrue(lifecycle.restoreFocus(bookmark))
        assertEquals(listOf("rotation", "process_recreated", "permission_revoked"), port.events)
        assertTrue(received.last().getBoolean("revalidate_permissions"))
        delegate.sendAccessibilityEvent(button, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
        assertTrue(port.focused.isEmpty()); bridge.close()
    }
    fun testFontAndDisplayConfigurationAreForwardedWithoutTextRemeasurement() = runTestOnUiThread {
        val base = instrumentation.targetContext
        val config = Configuration(base.resources.configuration).apply { fontScale = 2.0f; densityDpi = 320 }
        val scaled = base.createConfigurationContext(config)
        val bridge = NativeSemantics(FakePort())
        var environment: Pair<Float, Float>? = null
        AccessibilityLifecycle(bridge, {}, { font, density, _ -> environment = font to density }).configuration(scaled)
        assertEquals(2.0f, environment!!.first)
        assertEquals(scaled.resources.displayMetrics.density, environment!!.second)
        bridge.close()
    }
    fun testFailedExtensionMountClosesOwnedSemantics() = runTestOnUiThread {
        val port = FakePort().apply { prepared = tree(1).toString() }
        val bridge = NativeSemantics(port)
        val extension = object : SemanticViewAdapter.Extension {
            override fun mount(node: JSONObject, previous: View?): View = throw IllegalStateException("Synthetic mount failure")
            override fun dispose(view: View) {}
        }
        val adapter = SemanticViewAdapter(instrumentation.targetContext, { _, _, _ -> }, { _, _, _, _ -> }, { _, _ -> }, mapOf("fixture" to extension), bridge)
        val root = JSONObject().put("key", "root").put("element", JSONObject().put("kind", "fixture").put("props", JSONObject()))
        val bytes = JSONObject().put("schema", "rust-native.view.v3").put("instance", "fixture").put("revision", 1).put("root", root).toString().toByteArray()
        try { adapter.mount(LinearLayout(instrumentation.targetContext), bytes); fail("Mount must fail") } catch (_: IllegalStateException) {}
        assertEquals(1, port.closed)
    }
}
