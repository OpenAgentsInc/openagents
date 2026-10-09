// The Grid Gym's RESULTS board and trace viewer: the published
// Terminal-Bench results in the OpenAgents app's white-on-black style. Rust
// loads and verifies the publication, owns where the player is in the panel
// and the trace's playhead, and formats every figure with its labels
// (`gym_leaderboard::view`); this panel draws one screen's rows and sends
// the player's choices back. It never computes, sums, or relabels a number.
// It follows the iOS panel (`VerseResults.swift`, `VerseTrace.swift`).
package com.openagents.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.view.Gravity
import android.view.View
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.SeekBar
import org.json.JSONObject
import java.util.Locale

class ResultsPanel(private val context: Context, private val world: VerseSurface, private val rebuild: () -> Unit) {
    /** Only the trace's timeline shows, so the replay in the Gym is in view. */
    var watching = false
    private var seeking = false

    private fun choose(vararg values: Pair<String, Any?>) { world.results(json(*values)) }

    /**
     * Builds the panel's content for `view` (`results_view`), or a loading
     * row while it is null. `fixed` sits above the panel's scroll view and
     * holds a trace's header, timeline, and tabs, so they stay in view;
     * `scroll` is the scroll view, which the Agent tab moves to the
     * playhead's row.
     */
    fun build(view: JSONObject?, fixed: LinearLayout, scroll: ScrollView): View {
        val body = context.column()
        if (view == null) { body.add(context.label("Loading results…", 14f, Palette.SECONDARY, key = "results-loading")); return body }
        view.textOrNull("error")?.let { body.add(context.label(it, 14f, key = "results-error", selectable = true)) }
        val page = view.objectOrNull("page")
        when (page?.optString("screen")) {
            "boards" -> boards(body, page)
            "board" -> board(body, page)
            "attempt" -> attempt(body, page)
            "trace" -> trace(fixed, body, page, view.textOrNull("replay"), scroll)
            null -> {
                body.add(context.label(view.optString("status"), 14f, key = if (view.optBoolean("loading")) "results-loading" else null), 4)
                if (!view.optBoolean("loading") && view.optBoolean("active")) {
                    body.add(context.pill("Try again", "results-retry") { choose("do" to "retry") }, 10, -2)
                }
            }
            else -> body.add(context.label("This screen needs a newer app.", 14f))
        }
        return body
    }

    /** A tappable row that TalkBack reads as Rust's `accessibility` text. */
    private fun tappable(row: View, spoken: String, key: String, action: () -> Unit): View = row.apply {
        isClickable = true; isFocusable = true; tag = key
        contentDescription = spoken
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        (this as? android.view.ViewGroup)?.let { hideChildren(it) }
        setOnClickListener { action() }
    }

    private fun hideChildren(group: android.view.ViewGroup) {
        for (i in 0 until group.childCount) group.getChildAt(i).importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
    }

    // Screen 1: every board, in the publication's order.
    private fun boards(body: LinearLayout, page: JSONObject) {
        page.objectOrNull("summary")?.let { summary ->
            val row = context.column().apply {
                add(context.label(summary.getString("text"), 15f, bold = true))
                add(context.label(summary.getString("source"), 12f, Palette.SECONDARY), 4)
            }
            body.add(tappable(row, "${summary.getString("text")} ${summary.getString("source")}", "results-summary") {
                choose("do" to "board", "id" to summary.getString("board"))
            }, 4)
            body.addDivider()
        }
        body.add(context.label("Published Terminal-Bench results", 16f, bold = true), 12)
        for (row in page.getJSONArray("rows").objects()) {
            val item = context.column().apply {
                setPadding(0, context.dp(8), 0, context.dp(8))
                add(context.label(row.getString("title"), 15f, bold = true))
                add(context.label(row.getString("benchmark"), 12f, Palette.SECONDARY), 2)
                add(context.label(row.getString("headline"), 14f), 4)
                row.textOrNull("headline_note")?.let { add(context.label(it, 12f, Palette.SECONDARY), 2) }
                add(context.chips(row.optJSONArray("labels").chipList()), 6)
            }
            body.add(tappable(item, row.getString("accessibility"), "results-board-${row.getString("id")}") {
                choose("do" to "board", "id" to row.getString("id"))
            }, 4)
            body.addDivider(4)
        }
        page.textOrNull("footer")?.let { body.add(context.label(it, 11f, Palette.SECONDARY, key = "results-footer", mono = true), 10) }
        body.add(context.label("Each board is separate, in the order it was published. Scores aren't added up or ranked across boards.",
            12f, Palette.SECONDARY), 8)
    }

    // Screen 2: one board.
    private fun board(body: LinearLayout, page: JSONObject) {
        body.add(context.label(page.getString("summary"), 15f, bold = true, key = "results-board-summary"))
        body.add(context.label(page.getString("title"), 16f, bold = true), 10)
        body.add(context.label(page.getString("benchmark"), 12f, Palette.SECONDARY), 2)
        body.add(context.label(page.getString("headline"), 14f, key = "results-headline"), 6)
        page.textOrNull("headline_note")?.let { body.add(context.label(it, 12f, Palette.SECONDARY), 2) }
        body.add(context.chips(page.optJSONArray("labels").chipList()), 6)
        caveats(body, page)
        for (tally in page.getJSONArray("tallies").objects()) body.add(context.label(tally.getString("text"), 12f), 4)
        for (line in page.optJSONArray("spend").strings()) body.add(context.label(line, 12f), 2)
        reference(body, page.getJSONObject("reference"))
        val filters = context.row()
        for (filter in page.getJSONArray("filters").objects()) {
            val selected = filter.optBoolean("selected")
            filters.addView(context.text("${filter.getString("text")} (${filter.getInt("count")})", 12f).apply {
                setPadding(context.dp(10), context.dp(6), context.dp(10), context.dp(6))
                background = context.rounded(if (selected) 0x38FFFFFF else 0, 16f, 0x80FFFFFF.toInt())
                tag = "results-filter-${filter.getString("filter")}"
                isSelected = selected
                contentDescription = "${filter.getString("text")}, ${filter.getInt("count")}${if (selected) ", selected" else ""}"
                setOnClickListener { choose("do" to "filter", "filter" to filter.getString("filter")) }
            }, LinearLayout.LayoutParams(-2, -2).apply { marginEnd = context.dp(8) })
        }
        body.add(HorizontalScrollView(context).apply { isHorizontalScrollBarEnabled = false; addView(filters) }, 12)
        for (task in page.getJSONArray("tasks").objects()) {
            val header = context.row().apply {
                gravity = Gravity.CENTER_VERTICAL
                addView(context.label(task.getString("task"), 14f, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
                addView(context.label(task.getString("status"), 12f, Palette.SECONDARY))
                contentDescription = task.getString("accessibility")
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                hideChildren(this)
            }
            body.add(header, 12)
            body.add(context.label(task.getString("bar"), 11f, Palette.SECONDARY), 2)
            body.add(context.label(task.getString("knowledge"), 11f, Palette.SECONDARY), 2)
            for (cell in task.getJSONArray("attempts").objects()) {
                val beat = cell.optBoolean("beat"); val passed = cell.optBoolean("passed")
                val mark = if (beat) "★" else if (passed) "✓" else "✕"
                val item = context.column().apply {
                    setPadding(context.dp(8), context.dp(8), context.dp(8), context.dp(8))
                    background = context.rounded(0, 8f, if (beat) 0xCCFFFFFF.toInt() else 0x40FFFFFF)
                    add(context.label("$mark  ${cell.getString("series")}: ${cell.getString("text")}", 12f))
                    val labels = cell.optJSONArray("labels").chipList()
                    if (labels.isNotEmpty()) add(context.chips(labels, small = true), 4)
                    for (caveat in cell.optJSONArray("caveats")?.objects() ?: emptyList()) {
                        add(context.label(caveat.getString("text"), 11f, Palette.SECONDARY), 2)
                    }
                }
                body.add(tappable(item, cell.getString("accessibility"), "results-attempt-${cell.getString("id")}") {
                    choose("do" to "attempt", "id" to cell.getString("id"))
                }, 6)
            }
            body.addDivider(10)
        }
    }

    private fun caveats(body: LinearLayout, page: JSONObject) {
        val count = page.getInt("caveat_count")
        val open = page.optBoolean("caveats_open")
        val box = context.column().apply {
            setPadding(context.dp(10), context.dp(10), context.dp(10), context.dp(10))
            background = context.rounded(0, 10f, 0x59FFFFFF)
            add(context.label("Caveats ($count)", 13f, bold = true))
            for (caveat in page.getJSONArray("caveats").objects()) add(context.label(caveat.getString("text"), 12f, selectable = true), 4)
            if (count > 1) add(context.label(if (open) "Show the first caveat only" else "Show all $count caveats", 12f, Palette.LINK,
                key = "results-caveats").apply {
                setPadding(0, context.dp(8), 0, context.dp(4))
                setOnClickListener { choose("do" to "caveats", "open" to !open) }
            }, 2)
        }
        body.add(box, 10)
    }

    private fun reference(body: LinearLayout, reference: JSONObject) {
        body.add(context.label("Bar: ${reference.getString("name")}", 12f, bold = true), 10)
        body.add(context.label(reference.getString("rule"), 11f, Palette.SECONDARY), 2)
        val conditions = context.label(reference.getString("conditions"), 11f, selectable = true).apply { visibility = View.GONE }
        body.add(context.label("Reference conditions ›", 12f, Palette.LINK, key = "results-conditions").apply {
            setPadding(0, context.dp(6), 0, context.dp(6))
            setOnClickListener {
                val show = conditions.visibility != View.VISIBLE
                conditions.visibility = if (show) View.VISIBLE else View.GONE
                text = if (show) "Reference conditions ⌄" else "Reference conditions ›"
            }
        }, 2)
        body.add(conditions)
    }

    private fun header(body: LinearLayout, header: JSONObject) {
        val box = context.column().apply {
            tag = "results-header"
            add(context.label(header.getString("task"), 16f, bold = true))
            add(context.label("${header.getString("result")} · ${header.getString("beat")}", 14f, bold = true), 2)
            add(context.label("Cost ${header.getString("cost")}", 12f), 2)
            add(context.label("Time ${header.getString("time")}", 12f), 2)
            add(context.chips(header.optJSONArray("labels").chipList()), 4)
            contentDescription = header.getString("accessibility")
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            hideChildren(this)
        }
        body.add(box)
    }

    // Screen 3: one attempt.
    private fun attempt(body: LinearLayout, page: JSONObject) {
        header(body, page.getJSONObject("header"))
        body.add(context.label("${page.getString("board_title")} · ${page.getString("series")}", 12f, Palette.SECONDARY), 6)
        for (line in page.optJSONArray("numbers").strings()) body.add(context.label(line, 14f), 4)
        body.add(context.label(page.getString("misses"), 14f, bold = true, key = "results-misses"), 8)
        for (caveat in page.optJSONArray("caveats")?.objects() ?: emptyList()) body.add(context.label(caveat.getString("text"), 12f, Palette.SECONDARY), 4)
        reference(body, page.getJSONObject("reference"))
        val phases = page.optJSONArray("phases").strings()
        if (phases.isNotEmpty()) {
            body.add(context.label("Phases", 13f, bold = true), 8)
            for (phase in phases) body.add(context.label(phase, 12f), 2)
        }
        page.textOrNull("how_it_ended")?.let { body.add(context.label(it, 12f), 6) }
        page.textOrNull("jev")?.let { body.add(context.label(it, 12f), 6) }
        page.textOrNull("verifier")?.let { body.add(context.label("Verifier: $it", 12f), 6) }
        for (test in page.optJSONArray("failed_tests").strings()) body.add(context.label("Failed: $test", 11f, mono = true), 2)
        body.add(context.label(page.getString("trial"), 11f, Palette.SECONDARY, mono = true, selectable = true), 8)
        page.textOrNull("trace")?.let { trace ->
            body.add(context.pill("▶  $trace", "results-open-trace") { choose("do" to "trace") }, 12, -2)
        }
    }

    // Screen 4: one attempt's trace.
    private fun trace(top: LinearLayout, body: LinearLayout, page: JSONObject, replay: String?, scroll: ScrollView) {
        val headerJson = page.getJSONObject("header")
        if (watching) top.add(context.label("${headerJson.getString("task")} · ${headerJson.getString("result")} · ${headerJson.getString("beat")}",
            14f, bold = true))
        else header(top, headerJson)
        timeline(top, page.getJSONObject("clock"))
        val row = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
        row.addView(context.label(replay ?: "", 12f, key = "trace-replay"), LinearLayout.LayoutParams(0, -2, 1f))
        row.addView(context.label(if (watching) "Show the trace" else "Watch in the Gym", 12f, Palette.LINK, key = "trace-watch").apply {
            setPadding(context.dp(8), context.dp(10), 0, context.dp(10))
            setOnClickListener { watching = !watching; rebuild() }
        })
        top.add(row, 4)
        if (watching) return
        val tabs = context.row()
        for (tab in page.getJSONArray("tabs").objects()) {
            val selected = tab.optBoolean("selected") || tab.getString("tab") == page.getString("tab")
            tabs.addView(context.text(tab.getString("text"), 12f, if (selected) Palette.BACKGROUND else Palette.PRIMARY).apply {
                gravity = Gravity.CENTER
                setPadding(context.dp(6), context.dp(8), context.dp(6), context.dp(8))
                background = context.rounded(if (selected) Palette.PRIMARY else Palette.RAISED, 8f)
                isSelected = selected
                tag = "trace-tab-${tab.getString("tab")}"
                contentDescription = "${tab.getString("text")} tab${if (selected) ", selected" else ""}"
                enabled(tab.optBoolean("available", true))
                setOnClickListener { choose("do" to "tab", "tab" to tab.getString("tab")) }
            }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginEnd = context.dp(4) })
        }
        top.add(tabs, 8)
        // The running token counter at the playhead stays in view.
        page.objectOrNull("agent")?.let { top.add(context.label(it.getString("tokens"), 12f, key = "trace-tokens"), 6) }
        when (page.getString("tab")) {
            "jev" -> jev(body, page.objectOrNull("jev"))
            "briefing" -> page.objectOrNull("briefing")?.let { briefing ->
                briefing.textOrNull("cut")?.let { body.add(context.label(it, 11f, Palette.SECONDARY), 8) }
                body.add(context.label(briefing.getString("text"), 12f, mono = true, selectable = true), 6)
            } ?: body.add(context.label("No briefing in this trace.", 12f), 8)
            "agent" -> agent(body, page.objectOrNull("agent"), page.getJSONObject("clock").optInt("step"), scroll)
            else -> verifier(body, page.objectOrNull("verifier"))
        }
    }

    private fun timeline(body: LinearLayout, clock: JSONObject) {
        val marks = clock.optJSONArray("marks")?.let { list -> (0 until list.length()).map { list.getDouble(it) } } ?: emptyList()
        body.add(TimelineMarks(context, marks), 10)
        val fraction = clock.optDouble("fraction", 0.0).takeIf { it.isFinite() } ?: 0.0
        val step = clock.optInt("step"); val steps = clock.optInt("steps")
        body.add(SeekBar(context).apply {
            max = 1000
            progress = (fraction * 1000).toInt().coerceIn(0, 1000)
            tag = "trace-scrubber"
            contentDescription = "Trace timeline"
            stateDescriptionCompat("${clock.getString("text")}, step ${step + 1} of $steps")
            progressTintList = android.content.res.ColorStateList.valueOf(Palette.PRIMARY)
            thumbTintList = android.content.res.ColorStateList.valueOf(Palette.PRIMARY)
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar: SeekBar, value: Int, user: Boolean) = Unit
                override fun onStartTrackingTouch(bar: SeekBar) { seeking = true }
                override fun onStopTrackingTouch(bar: SeekBar) {
                    seeking = false
                    choose("do" to "seek", "fraction" to bar.progress / 1000.0)
                }
            })
        })
        val controls = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
        val playing = clock.optBoolean("playing")
        controls.addView(control("\u23EE\uFE0E", "Step back", "trace-step-back") { choose("do" to "step", "forward" to false) })
        controls.addView(control(if (playing) "\u275A\u275A" else "\u25B6\uFE0E", if (playing) "Pause" else "Play", "trace-play") {
            choose("do" to "play", "playing" to !playing)
        })
        controls.addView(control("\u23ED\uFE0E", "Step forward", "trace-step-forward") { choose("do" to "step", "forward" to true) })
        controls.addView(View(context), LinearLayout.LayoutParams(0, 1, 1f))
        controls.addView(context.label("${clock.getString("text")} · step ${step + 1} of $steps", 12f))
        body.add(controls, 2)
    }

    /** True while the person drags the scrubber, so a new revision doesn't take it away. */
    val scrubbing get() = seeking

    private fun control(glyph: String, spoken: String, key: String, action: () -> Unit) =
        context.text(glyph, 18f).apply {
            gravity = Gravity.CENTER; tag = key; contentDescription = spoken
            minWidth = context.dp(44); minHeight = context.dp(44)
            setOnClickListener { action() }
        }

    private fun jev(body: LinearLayout, jev: JSONObject?) {
        if (jev == null) { body.add(context.label("No Jev decision in this trace.", 12f), 8); return }
        body.add(context.label(jev.getString("summary"), 12f), 8)
        body.add(context.label("Candidates", 13f, bold = true), 8)
        val keep = jev.getDouble("keep_threshold"); val flag = jev.getDouble("flag_threshold")
        for (candidate in jev.getJSONArray("candidates").objects()) {
            val kept = candidate.optBoolean("kept")
            val row = context.column().apply {
                val line = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
                val name = candidate.textOrNull("title") ?: candidate.getString("id")
                line.addView(context.label("${candidate.getInt("rank")}. $name", 12f, bold = kept), LinearLayout.LayoutParams(0, -2, 1f))
                if (candidate.optBoolean("own")) line.addView(context.text("own", 11f).apply {
                    setPadding(context.dp(5), 0, context.dp(5), 0); background = context.rounded(0, 8f, 0xCCFFFFFF.toInt())
                }, LinearLayout.LayoutParams(-2, -2).apply { marginEnd = context.dp(6) })
                line.addView(context.label(if (kept) "kept" else candidate.getString("fate"), 11f, Palette.SECONDARY))
                add(line)
                addView(ProbabilityBar(context, candidate.getDouble("p"), keep, kept), LinearLayout.LayoutParams(-1, context.dp(18)).apply { topMargin = context.dp(2) })
                contentDescription = candidate.getString("accessibility")
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                hideChildren(this)
            }
            body.add(row, 6)
        }
        body.add(context.label("Requirements", 13f, bold = true), 12)
        for (requirement in jev.getJSONArray("requirements").objects()) {
            val flagged = requirement.optBoolean("flagged")
            val row = context.column().apply {
                add(context.label(requirement.getString("text"), 12f, bold = flagged))
                addView(ProbabilityBar(context, requirement.getDouble("p"), flag, flagged), LinearLayout.LayoutParams(-1, context.dp(18)).apply { topMargin = context.dp(2) })
                contentDescription = requirement.getString("accessibility")
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                hideChildren(this)
            }
            body.add(row, 6)
        }
    }

    private fun agent(body: LinearLayout, agent: JSONObject?, step: Int, scroll: ScrollView) {
        if (agent == null) return
        val page = agent.getInt("page"); val pages = agent.getInt("pages")
        if (pages > 1) {
            val pager = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
            pager.addView(context.label("Earlier", 12f, Palette.LINK, key = "trace-earlier").apply {
                setPadding(0, context.dp(8), context.dp(8), context.dp(8)); enabled(page > 0)
                setOnClickListener { if (page > 0) choose("do" to "page", "page" to page - 1) }
            })
            pager.addView(context.label("Page ${page + 1} of $pages", 12f).apply { gravity = Gravity.CENTER }, LinearLayout.LayoutParams(0, -2, 1f))
            pager.addView(context.label("Later", 12f, Palette.LINK, key = "trace-later").apply {
                setPadding(context.dp(8), context.dp(8), 0, context.dp(8)); enabled(page + 1 < pages)
                setOnClickListener { if (page + 1 < pages) choose("do" to "page", "page" to page + 1) }
            })
            body.add(pager, 6)
        }
        var current: View? = null
        for (row in agent.getJSONArray("rows").objects()) {
            val index = row.getInt("index")
            val kind = row.getString("kind")
            val expandable = row.optBoolean("expandable")
            val output = row.objectOrNull("output")
            val item = context.column().apply {
                setPadding(context.dp(6), context.dp(6), context.dp(6), context.dp(6))
                if (row.optBoolean("current")) background = context.rounded(0, 6f, 0xE6FFFFFF.toInt())
                alpha = if (row.optBoolean("reached")) 1f else 0.45f
                val top = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
                top.addView(context.label(row.getString("at"), 11f, Palette.SECONDARY, mono = true))
                top.addView(context.label(kind.replace('_', ' '), 11f, bold = true), LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(6) })
                if (row.has("exit_code") && !row.isNull("exit_code")) top.addView(context.label("exit ${row.getLong("exit_code")}", 11f, mono = true),
                    LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(6) })
                add(top)
                add(context.label(row.getString("text"), 12f, mono = kind == "command").apply {
                    maxLines = if (kind == "say") 6 else 4; ellipsize = android.text.TextUtils.TruncateAt.END
                }, 2)
                row.textOrNull("cut")?.let { add(context.label(it, 11f, Palette.SECONDARY), 2) }
                output?.let {
                    add(context.label(it.getString("text"), 11f, mono = true, selectable = true).apply {
                        setPadding(context.dp(6), context.dp(6), context.dp(6), context.dp(6))
                        background = context.rounded(0xFF1A1A1A.toInt(), 6f)
                    }, 4)
                }
                tag = "trace-step-$index"
                contentDescription = row.getString("accessibility") +
                    if (expandable) (if (output == null) ". Double-tap to show the output." else ". Double-tap to hide the output.") else ""
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                hideChildren(this)
                if (expandable) {
                    isClickable = true
                    setOnClickListener { choose("do" to "expand", "index" to (if (output == null) index else null)) }
                }
            }
            body.add(item, 4)
            if (index == step) current = item
        }
        // The Agent tab follows the playhead's row.
        current?.let { row -> scroll.post { scroll.smoothScrollTo(0, (rowTop(row, scroll) - scroll.height / 2 + row.height / 2).coerceAtLeast(0)) } }
    }

    private fun rowTop(view: View, ancestor: View): Int {
        var top = 0; var current: View? = view
        while (current != null && current !== ancestor) { top += current.top; current = current.parent as? View }
        return top
    }

    private fun verifier(body: LinearLayout, verifier: JSONObject?) {
        if (verifier == null) { body.add(context.label("No verifier result in this trace.", 12f), 8); return }
        body.add(context.label(verifier.getString("summary"), 14f, bold = true), 8)
        for (test in verifier.getJSONArray("tests").objects()) {
            val row = context.row().apply {
                gravity = Gravity.CENTER_VERTICAL
                addView(context.label(if (test.optBoolean("passed")) "✓" else "✕", 12f), LinearLayout.LayoutParams(context.dp(18), -2))
                addView(context.label(test.getString("name"), 12f, mono = true), LinearLayout.LayoutParams(0, -2, 1f))
                addView(context.label(test.getString("status"), 11f, Palette.SECONDARY))
                contentDescription = "${test.getString("name")}, ${test.getString("status")}"
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
                hideChildren(this)
            }
            body.add(row, 4)
        }
        body.add(context.label("Output tail", 13f, bold = true), 12)
        val tail = verifier.getJSONObject("output_tail")
        tail.textOrNull("cut")?.let { body.add(context.label(it, 11f, Palette.SECONDARY), 2) }
        body.add(context.label(tail.getString("text"), 11f, mono = true, selectable = true), 4)
    }
}

private fun View.stateDescriptionCompat(value: String) {
    if (android.os.Build.VERSION.SDK_INT >= 30) stateDescription = value
}

/** The timeline's step marks, as short ticks above the scrubber. */
private class TimelineMarks(context: Context, private val marks: List<Double>) : View(context) {
    private val ink = Paint().apply { color = 0x59FFFFFF; strokeWidth = context.dp(1).toFloat() }
    init { minimumHeight = context.dp(6); importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) =
        setMeasuredDimension(MeasureSpec.getSize(widthMeasureSpec), context.dp(6))
    override fun onDraw(canvas: Canvas) {
        // The SeekBar's track is inset by its padding; match it.
        val inset = context.dp(16).toFloat()
        for (mark in marks) if (mark.isFinite()) {
            val x = inset + mark.coerceIn(0.0, 1.0).toFloat() * (width - 2 * inset)
            canvas.drawLine(x, 0f, x, height.toFloat(), ink)
        }
    }
}

/** A probability as a bar, with the keep or flag threshold as a line. */
private class ProbabilityBar(context: Context, private val p: Double, private val threshold: Double,
                             private val strong: Boolean) : View(context) {
    private val track = Paint().apply { color = 0x1FFFFFFF }
    private val fill = Paint().apply { color = if (strong) 0xD9FFFFFF.toInt() else 0x66FFFFFF }
    private val line = Paint().apply { color = Palette.PRIMARY }
    private val label = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Palette.PRIMARY; textSize = context.dpf(10f); typeface = PaperMono.typeface(context); textAlign = Paint.Align.RIGHT
    }
    init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }
    override fun onDraw(canvas: Canvas) {
        val bar = context.dpf(6f)
        val top = height - bar
        canvas.drawRect(0f, top, width.toFloat(), height.toFloat(), track)
        canvas.drawRect(0f, top, (p.coerceIn(0.0, 1.0) * width).toFloat(), height.toFloat(), fill)
        val x = (threshold.coerceIn(0.0, 1.0) * width).toFloat()
        canvas.drawRect(x - 1, top, x + 1, height.toFloat(), line)
        canvas.drawText(String.format(Locale.US, "%.2f", p), width.toFloat(), top - context.dpf(2f), label)
    }
}
