package com.openagents.app.composer

import org.json.JSONObject
import java.nio.charset.StandardCharsets

/** Loads the bounded JSON fixture used by the Android composer preview. */
object ComposerFixtureLoader {
    fun load(json: String): ComposerFrame {
        require(json.toByteArray(StandardCharsets.UTF_8).size <= MAX_FIXTURE_BYTES)
        val source = JSONObject(json)
        require(source.getString("schema") == SCHEMA)
        val frame = ComposerFrame(
            stamp = ComposerEditorStamp(
                instance = source.getString("instance"),
                viewRevision = source.getLong("revision"),
                node = source.getString("composer_node"),
                inputToken = source.getString("input_token"),
                editingLifetime = source.getString("editing_lifetime"),
                editorRevision = source.getLong("editor_revision"),
            ),
            initialDraft = source.getString("draft"),
            maxBytes = source.getInt("max_bytes"),
            placeholder = source.getString("placeholder"),
            enabled = source.getBoolean("enabled"),
            busy = source.getBoolean("busy"),
            primaryLabel = source.getString("primary_label"),
            stopAction = source.optJSONObject("stop_action")?.let(::action),
            sendChoices = source.getJSONArray("send_choices").let { choices ->
                List(choices.length()) { index ->
                    val choice = choices.getJSONObject(index)
                    ComposerChoice(choice.getString("token"), choice.getString("label"))
                }
            },
            contextChips = source.getJSONArray("context_chips").let { chips ->
                List(chips.length()) { index ->
                    val chip = chips.getJSONObject(index)
                    ComposerChip(chip.getString("key"), chip.getString("label"))
                }
            },
            attachments = source.getJSONArray("attachments").let { attachments ->
                List(attachments.length()) { index ->
                    val attachment = attachments.getJSONObject(index)
                    ComposerAttachment(
                        key = attachment.getString("key"),
                        label = attachment.getString("label"),
                        removeAction = attachment.optJSONObject("remove_action")?.let(::action),
                    )
                }
            },
            attachmentActions = actions(source, "attachment_actions"),
            mentionSuggestions = actions(source, "mention_suggestions"),
            sendOnEnter = source.getBoolean("send_on_enter"),
            sharedCardsJson = source.optJSONObject("shared_cards")?.toString(),
        )
        require(frame.sendChoices.size <= MAX_CHOICES)
        require(frame.attachments.size <= MAX_ATTACHMENTS)
        require(frame.initialDraft.toByteArray(StandardCharsets.UTF_8).size <= frame.maxBytes)
        frame.sharedCardsJson?.let {
            require(it.toByteArray(StandardCharsets.UTF_8).size <= MAX_SHARED_VIEW_BYTES)
        }
        return frame
    }

    private fun actions(source: JSONObject, key: String) = source.getJSONArray(key).let { values ->
        require(values.length() <= MAX_ACTIONS)
        List(values.length()) { index -> action(values.getJSONObject(index)) }
    }

    private fun action(value: JSONObject) = ComposerAction(
        node = value.getString("node"),
        label = value.getString("label"),
        enabled = value.optBoolean("enabled", true),
    )

    private const val SCHEMA = "openagents.android.composer-fixture.v1"
    private const val MAX_FIXTURE_BYTES = 512 * 1024
    private const val MAX_SHARED_VIEW_BYTES = 512 * 1024
    private const val MAX_CHOICES = 4
    private const val MAX_ATTACHMENTS = 10
    private const val MAX_ACTIONS = 16
}
