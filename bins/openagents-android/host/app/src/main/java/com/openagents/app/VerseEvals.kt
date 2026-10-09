// The Grid Gym's EVALS board: published extension eval results grouped by
// test set and tool, with their checks and credit, and the notes players'
// agents trade in the Gym, as in the iOS Verse tab. Rust reads and verifies
// every record, groups the results, renders every note from the results it
// cites, and decides when the player's agent speaks (`verse::gym_hall`); this
// panel draws the screen and sends the Compare notes switch back. It never
// computes a number or writes a note.
package com.openagents.app

import android.content.Context
import android.view.View
import android.widget.LinearLayout
import android.widget.Switch
import org.json.JSONObject

internal class EvalsPanel(
    private val context: Context,
    private val world: VerseSurface,
    /** Train Coder: opt into the Gym; Rust opens its intro on the Chat tab. */
    private val train: () -> Unit,
) {
    /** The `evals_view` Rust sent, or a line while it reads. */
    fun build(view: JSONObject?): View = context.column().apply {
        if (view == null) {
            add(context.label("Reading results…", 14f, Palette.SECONDARY, key = "evals-loading"))
            return@apply
        }
        // Train Coder opened a test of the Gym's sample plugins, which are no longer shown; it comes back when the Gym has a real plugin to test.
        // add(context.label("Train Coder", 16f, bold = true, key = "evals-train").apply {
        //     isClickable = true; isFocusable = true
        //     setPadding(0, context.dp(8), 0, context.dp(8))
        //     setOnClickListener { train() }
        // })
        // addDivider(12)
        // Compare notes: agents in the Gym.
        val toggle = context.row().apply {
            addView(context.column().apply {
                addView(context.label("Compare notes", 16f, bold = true))
                addView(context.label("Agents in the Gym", 12f, Palette.SECONDARY))
            }, LinearLayout.LayoutParams(0, -2, 1f))
            @Suppress("DEPRECATION")
            addView(Switch(context).apply {
                isChecked = view.optBoolean("notes_on")
                tag = "evals-notes-toggle"
                contentDescription = "Compare notes"
                setOnCheckedChangeListener { _, on -> world.evals(json("do" to "notes", "on" to on)) }
            })
        }
        add(toggle)
        add(context.label(view.optString("notes_note"), 12f, Palette.SECONDARY), 4)
        val notes = view.optJSONArray("notes")?.objects().orEmpty()
        if (notes.isEmpty()) add(context.label(view.optString("notes_empty"), 14f, key = "evals-notes-empty"), 6)
        for (note in notes) {
            val who = if (note.optBoolean("mine")) "Our agent" else "${note.optString("author_tag")}'s agent"
            val heading = note.textOrNull("answers_tag")?.let { "$who, answering $it's agent" } ?: who
            add(context.column().apply {
                setPadding(context.dp(10), context.dp(8), context.dp(10), context.dp(8))
                background = context.rounded(if (note.optBoolean("mine")) 0x1FFFFFFF else 0x0DFFFFFF, 10f)
                addView(context.label(heading, 12f, Palette.SECONDARY, mono = true))
                addView(context.label(note.optString("text"), 14f))
            }, 6)
        }
        addDivider(12)
        add(context.label("Published results", 16f, bold = true), 10)
        val state = view.optString("state")
        if (state != "ready") add(context.label(
            if (state == "connecting") "Connecting to ${view.optString("relay")}…" else "Reading ${view.optString("relay")}…",
            12f, Palette.SECONDARY, key = "evals-state"), 4)
        val groups = view.optJSONObject("board")?.optJSONArray("groups")?.objects().orEmpty()
        if (groups.isEmpty() && state == "ready") add(context.label(view.optString("empty"), 14f, key = "evals-empty"), 6)
        for (group in groups) {
            add(context.label(group.optString("test_set"), 16f, bold = true), 12)
            add(context.label("Test set by ${group.optString("author_tag")}", 12f, Palette.SECONDARY))
            for (row in group.optJSONArray("rows")?.objects().orEmpty()) add(row(row), 8)
            val more = group.optInt("more")
            if (more > 0) add(context.label("$more older results not shown", 12f, Palette.SECONDARY), 4)
            addDivider(10)
        }
        add(context.label(view.optString("note"), 12f, Palette.SECONDARY), 10)
    }

    private fun row(row: JSONObject): View = context.column().apply {
        tag = "evals-row-${row.optString("id").take(8)}"
        add(context.row().apply {
            addView(context.label(row.optString("tool"), 15f, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
            addView(context.label(row.optString("verdict_words"), 12f, bold = true).apply {
                setPadding(context.dp(8), context.dp(2), context.dp(8), context.dp(2))
                background = context.rounded(0, 10f, if (row.optString("verdict") == "pass") 0xE6FFFFFF.toInt() else 0x66FFFFFF)
            })
        })
        add(context.label(row.optString("headline"), 14f))
        val who = if (row.optBoolean("mine")) "You (${row.optString("trainer_tag")})" else row.optString("trainer_tag")
        val extra = buildList {
            add(who)
            if (row.optBoolean("hosted")) add("run on our computers")
            if (row.optLong("credit_xp") > 0) add("${row.optLong("credit_xp")} XP credit")
        }.joinToString(" · ")
        add(context.label(extra, 12f, Palette.SECONDARY, mono = true))
        row.textOrNull("checks")?.let { add(context.label(it, 12f)) }
    }
}
