package com.openagents.app

import android.test.InstrumentationTestCase
import android.view.View
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject

/** Bounded offline fixtures; run as Android instrumentation when the app shell is available. */
class SemanticViewAdapterTest : InstrumentationTestCase() {
    private fun node(key: String, kind: String, props: JSONObject) = JSONObject()
        .put("key", key).put("style", JSONObject())
        .put("element", JSONObject().put("kind", kind).put("props", props))

    private fun fixture(revision: Long, enabled: Boolean = true): ByteArray {
        val field = node("name", "field", JSONObject().put("label", "Agent name")
            .put("placeholder", "Name").put("value", "Foreman").put("secret", false)
            .put("multiline", false).put("enabled", enabled).put("max_bytes", 64))
        val save = node("save", "button", JSONObject().put("label", "Save").put("enabled", enabled))
        val heading = node("heading", "text", JSONObject().put("value", "Agents").put("role", "heading"))
        val root = node("home", "stack", JSONObject().put("axis", "vertical")
            .put("children", JSONArray().put(heading).put(field).put(save)))
        return JSONObject().put("schema", "rust-native.view.v3").put("instance", "fixture")
            .put("revision", revision).put("root", root).toString().toByteArray(Charsets.UTF_8)
    }

    fun testKeysRolesUpdatesAndDisposal() = runTestOnUiThread {
        val context = instrumentation.targetContext
        val parent = LinearLayout(context)
        val actions = mutableListOf<String>()
        val changes = mutableListOf<String>()
        val acknowledgments = mutableListOf<Long>()
        val adapter = SemanticViewAdapter(context,
            { instance, revision, key -> actions.add("$instance:$revision:$key") },
            { _, revision, key, value -> changes.add("$revision:$key:$value") },
            { _, revision -> acknowledgments.add(revision) })
        adapter.mount(parent, fixture(1))
        val field = parent.findViewWithTag<EditText>("name")
        val save = parent.findViewWithTag<View>("save")
        assertEquals("Agent name", field.contentDescription)
        assertTrue(parent.findViewWithTag<TextView>("heading").isAccessibilityHeading)
        field.setText("Test agent")
        assertEquals(listOf("1:name:Test agent"), changes)
        save.performClick()
        assertEquals(listOf("fixture:1:save"), actions)
        adapter.mount(parent, fixture(2, false))
        assertSame(field, parent.findViewWithTag<EditText>("name"))
        assertFalse(field.isEnabled)
        assertEquals(listOf(1L, 2L), acknowledgments)
        adapter.dispose()
        assertEquals(0, parent.childCount)
        assertEquals("", field.text.toString())
        save.performClick()
        field.setText("Detached")
        assertEquals(1, actions.size)
        assertEquals(1, changes.size)
        try {
            adapter.mount(parent, fixture(3))
            fail("A disposed surface must refuse mounts")
        } catch (_: IllegalStateException) { }
    }

    fun testUnsupportedViewsDoNotAcknowledgeOrReplaceControls() = runTestOnUiThread {
        val parent = LinearLayout(instrumentation.targetContext)
        val acknowledgments = mutableListOf<Long>()
        val adapter = SemanticViewAdapter(instrumentation.targetContext, { _, _, _ -> },
            { _, _, _, _ -> }, { _, revision -> acknowledgments.add(revision) })
        adapter.mount(parent, fixture(1))
        val root = parent.getChildAt(0)
        val invalid = JSONObject(String(fixture(2), Charsets.UTF_8))
        invalid.getJSONObject("root").getJSONObject("element").put("kind", "unknown")
        try {
            adapter.mount(parent, invalid.toString().toByteArray(Charsets.UTF_8))
            fail("Unsupported elements must refuse the mount")
        } catch (_: IllegalArgumentException) { }
        assertEquals(listOf(1L), acknowledgments)
        assertSame(root, parent.getChildAt(0))
        adapter.dispose()
    }
}
