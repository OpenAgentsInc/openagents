package com.openagents.app.composer

import org.json.JSONObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ComposerMentionQueryTest {
    @Test
    fun queryFixtureCoversWhitespaceUnicodeAndEmailBoundaries() {
        val fixture = javaClass.classLoader!!.getResourceAsStream("fixtures/composer-mentions-v1.json")!!
            .bufferedReader()
            .use { it.readText() }
        val root = JSONObject(fixture)
        assertEquals("openagents.android.composer-mentions.v1", root.getString("schema"))
        val cases = root.getJSONArray("queries")
        for (index in 0 until cases.length()) {
            val item = cases.getJSONObject(index)
            val actual = ComposerMentionQuery.active(item.getString("text"), item.getInt("cursor_utf16"))
            if (item.optBoolean("no_match")) {
                assertNull(actual)
            } else {
                requireNotNull(actual)
                assertEquals(item.getInt("start_utf16"), actual.startUtf16)
                assertEquals(item.getInt("end_utf16"), actual.endUtf16)
                assertEquals(item.getString("query"), actual.query)
            }
        }
    }

    @Test
    fun selectedFileUsesRustCanonicalLinkForTextInsertion() {
        val fixture = javaClass.classLoader!!.getResourceAsStream("fixtures/composer-mentions-v1.json")!!
            .bufferedReader()
            .use { it.readText() }
        val selection = JSONObject(fixture).getJSONObject("selection")
        val file = ComposerMentionFile(selection.getString("file_path"), selection.getBoolean("is_directory"))
        val rust = object : ComposerMentionPort {
            override suspend fun searchFiles(query: String) = emptyList<ComposerMentionFile>()
            override fun fileMentionLink(file: ComposerMentionFile) = selection.getString("canonical_link")
        }
        val canonical = rust.fileMentionLink(file)
        val active = ComposerMentionQuery.active("@${selection.getString("query")}", selection.getString("query").length + 1)!!
        val inserted = "@${selection.getString("query")}".replaceRange(active.startUtf16, active.endUtf16, canonical)
        assertEquals(selection.getString("expected_text"), inserted)
    }
}
