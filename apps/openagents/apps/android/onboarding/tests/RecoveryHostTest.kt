package com.openagents.app.onboarding

import android.app.Dialog
import android.test.InstrumentationTestCase
import android.view.View
import android.view.WindowManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import uniffi.openagents_core.RecoveryAuthentication
import uniffi.openagents_core.RecoveryFrame

/** Authored synthetic port results; no device credential, clipboard, account, or network calls. */
class RecoveryHostTest : InstrumentationTestCase() {
    private class Port : RecoveryPort {
        var revision = 0L
        var accepted = 0
        var closed = 0
        var redacted = 0
        private fun frame(secret: Boolean, auth: Boolean = false): RecoveryFrame {
            revision++
            val text = JSONObject().put("key", "body").put("element", JSONObject().put("kind", "text").put("props", JSONObject().put("value", if (secret) "synthetic words" else "Your ID").put("role", "body")))
            val root = JSONObject().put("key", "root").put("element", JSONObject().put("kind", "stack").put("props", JSONObject().put("axis", "vertical").put("children", JSONArray().put(text))))
            val json = JSONObject().put("schema", "rust-native.view.v3").put("instance", "android-recovery").put("revision", revision).put("root", root).toString().toByteArray()
            return RecoveryFrame(json, secret || auth, if (auth) RecoveryAuthentication(1uL, "Unlock your device", "Confirm it's you.", 60_000u) else null, false, null)
        }
        override fun start(device: String) = frame(false, true)
        override fun render() = frame(false)
        override fun acknowledge(instance: String, revision: Long) {}
        override fun activate(instance: String, revision: Long, key: String) = frame(false)
        override fun change(instance: String, revision: Long, key: String, value: String) {}
        override fun authenticated(token: ULong, accepted: Boolean): RecoveryFrame { if (accepted) this.accepted++; return frame(accepted) }
        override fun redact() { redacted++ }
        override fun close() { closed++ }
    }
    private class Auth : DeviceAuthentication {
        var callback: ((Boolean) -> Unit)? = null
        override fun authenticate(title: String, description: String, result: (Boolean) -> Unit): AutoCloseable { callback = result; return AutoCloseable {} }
    }
    private class Clipboard : SecretClipboard {
        var copies = 0
        var closes = 0
        override fun copy(text: String, clearAfterMs: Long) { copies++ }
        override fun close() { closes++ }
    }
    fun testBackgroundAuthenticationResultWaitsForForeground() {
        val port = Port(); val auth = Auth(); val clipboard = Clipboard()
        lateinit var host: RecoveryHost; lateinit var parent: LinearLayout
        runTestOnUiThread {
            parent = LinearLayout(instrumentation.targetContext)
            host = RecoveryHost(parent, Dialog(parent.context).window!!, port, auth, clipboard, { it() }, {})
            host.start("synthetic")
        }
        instrumentation.waitForIdleSync()
        runTestOnUiThread { host.background(); auth.callback!!(true); assertEquals(0, parent.childCount) }
        instrumentation.waitForIdleSync()
        assertEquals(0, port.accepted)
        runTestOnUiThread { host.foreground() }
        instrumentation.waitForIdleSync()
        assertEquals(1, port.accepted)
        runTestOnUiThread { host.close(); assertEquals(0, parent.childCount) }
        assertEquals(0, clipboard.copies)
    }
    fun testClosedScreenRejectsLateDeviceCallbackAndReleasesViews() {
        val port = Port(); val auth = Auth(); val clipboard = Clipboard()
        lateinit var host: RecoveryHost
        runTestOnUiThread { val parent = LinearLayout(instrumentation.targetContext); host = RecoveryHost(parent, Dialog(parent.context).window!!, port, auth, clipboard, { it() }, {}); host.start("synthetic") }
        instrumentation.waitForIdleSync()
        runTestOnUiThread { host.close(); auth.callback!!(true) }
        instrumentation.waitForIdleSync()
        assertEquals(0, port.accepted); assertEquals(1, port.closed); assertTrue(port.redacted > 0)
    }
    fun testSecretScreenPreventsSelectionAutofillAndSavedInputAndClearsOwnedFlag() = runTestOnUiThread {
        val parent = LinearLayout(instrumentation.targetContext)
        val label = TextView(parent.context).apply { text = "synthetic words"; setTextIsSelectable(true) }
        val input = EditText(parent.context).apply { setText("synthetic typed words") }
        parent.addView(label); parent.addView(input)
        val window = Dialog(parent.context).window!!
        val screen = SecretScreen(window, parent); screen.protect(); screen.harden()
        assertTrue(window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0)
        assertFalse(label.isTextSelectable); assertFalse(input.isSaveEnabled)
        assertEquals(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS, input.importantForAutofill)
        screen.close(); assertEquals("", label.text.toString()); assertEquals("", input.text.toString())
        assertEquals(0, window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE)
    }
    fun testExistingSecureFlagIsPreserved() = runTestOnUiThread {
        val parent = LinearLayout(instrumentation.targetContext); val window = Dialog(parent.context).window!!
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        val screen = SecretScreen(window, parent); screen.protect(); screen.close()
        assertTrue(window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0)
    }
}
