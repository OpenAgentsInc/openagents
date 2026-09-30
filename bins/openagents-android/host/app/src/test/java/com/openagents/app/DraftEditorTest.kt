package com.openagents.app

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DraftEditorTest {
    /** Records requests and answers with the state Rust would, for one draft. */
    private class Fake : DraftEditor.Native {
        val requests = mutableListOf<JSONObject>()
        var revision = 0L
        var text = ""
        override fun create() = 7L
        override fun destroy(handle: Long) { requests += JSONObject().put("op", "destroyed") }
        override fun call(handle: Long, request: String): String {
            val json = JSONObject(request)
            requests += json
            when (json.getString("op")) {
                "mount" -> { text = json.optString("draft", ""); revision = 0 }
                "apply" -> {
                    val stamp = json.getJSONObject("stamp")
                    if (stamp.getLong("revision") != revision) return reply("stale")
                    val change = json.getJSONObject("change")
                    if (change.getString("op") == "sync") text = change.getString("text")
                    if (change.getString("op") == "delete") text = text.dropLast(2)
                    revision += 1
                }
            }
            return reply(null, replaced = json.getString("op") == "mount")
        }
        private fun reply(error: String?, replaced: Boolean = false): String = JSONObject()
            .put("state", JSONObject()
                .put("stamp", JSONObject().put("token", "composer-1").put("lifetime", 1).put("revision", revision))
                .put("text", text).put("selection", org.json.JSONArray(listOf(text.length, text.length)))
                .put("marked", JSONObject.NULL).put("can_undo", revision > 0).put("can_redo", false))
            .also { if (error != null) it.put("error", error) }
            .also { if (replaced) it.put("replaced", true) }
            .toString()
    }

    @Test fun editsNameTheCurrentStampAndShowRustsDraft() {
        val fake = Fake()
        val editor = DraftEditor(fake)
        assertTrue(editor.live)
        assertTrue(editor.mount("composer-1", 64, "hi"))
        assertEquals("hi", editor.state!!.text)
        val typed = editor.sync("hi 👋", 5, 5, null)!!
        assertEquals("hi 👋", typed.text)
        assertEquals(1L, typed.revision)
        val sent = fake.requests.last()
        assertEquals("apply", sent.getString("op"))
        assertEquals(0L, sent.getJSONObject("stamp").getLong("revision"))
        assertEquals("sync", sent.getJSONObject("change").getString("op"))
        assertTrue(sent.getJSONObject("change").isNull("marked"))
        // The delete key removes the whole emoji: both UTF-16 units.
        assertEquals("hi ", editor.delete(true)!!.text)
        assertTrue(editor.state!!.canUndo)
        editor.close()
        assertFalse(editor.live)
        assertEquals("destroyed", fake.requests.last().getString("op"))
        assertNull(DraftEditor(object : DraftEditor.Native {
            override fun create(): Long = throw UnsatisfiedLinkError()
            override fun call(handle: Long, request: String) = ""
            override fun destroy(handle: Long) = Unit
        }).apply { mount("composer-1", 64, null) }.state)
    }

    @Test fun composingRangesAndRefusalsParse() {
        val state = DraftEditor.parse(JSONObject("""{"stamp":{"token":"t","lifetime":2,"revision":9},
            "text":"say にほ","selection":[6,4],"marked":[4,6],"can_undo":true,"can_redo":false}"""))
        assertEquals(4 to 6, state.marked)
        assertEquals(4, state.start)
        assertEquals(6, state.end)
    }

    @Test fun syntaxSpansParseAndSkipMalformedRows() {
        val spans = Syntax.parse("""[[0,2,[180,142,173,255]],[2,1,[1,2,3]],[3,4,[230,232,235,255]]]""")
        assertEquals(2, spans.size)
        assertEquals(0, spans[0].start)
        assertEquals(2, spans[0].length)
        assertEquals(3, spans[1].start)
    }
}
