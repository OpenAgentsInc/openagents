// The Grid's Gym board in the Verse tab: the same board as Coder's Gym, in
// the OpenAgents app's white-on-black style. Rust owns the board's
// subscriptions, verified snapshots, recipe selection, authority, and launch
// identity; this panel renders what Rust sends and forwards choices. The
// Keystore-encrypted store keeps the Gym connection's exact bytes
// (DeviceKey.Purpose.GYM) and nothing else.
package com.openagents.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.view.View
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ProgressBar
import org.json.JSONObject
import java.util.Locale

class GymPanel(private val context: Context, private val world: VerseSurface) {
    private var configuring = false
    private var codeDraft = ""

    /** Builds the board's content. `board` is null while it loads. */
    fun build(board: JSONObject?): View {
        val body = context.column()
        if (board == null) {
            body.add(context.label("Loading Gym board…", 14f, Palette.SECONDARY, key = "gym-loading"))
            return body
        }
        body.add(context.label(board.optString("status"), 12f, Palette.SECONDARY, key = "gym-status"))
        if (board.optBoolean("stale")) body.add(context.label("This board is out of date, so new runs can't start.", 12f), 4)
        val problem = board.textOrNull("error") ?: world.snapshot?.textOrNull("error") ?: world.gymStorageError
        problem?.let { body.add(context.label(it.take(2048), 14f, key = "gym-error", selectable = true), 6) }
        when {
            !board.optBoolean("configured") || configuring -> connection(body, board)
            board.objectOrNull("selected_run") != null -> {
                body.add(context.pill("All runs", "gym-all-runs") { world.send(json("action" to "gym_close_detail")) }, 12, -2)
                details(body, board.getJSONObject("selected_run"))
            }
            board.objectOrNull("selected_recipe") != null -> {
                body.add(context.pill("All runs", "gym-all-runs") { world.send(json("action" to "gym_close_detail")) }, 12, -2)
                recipe(body, board.getJSONObject("selected_recipe"), board)
            }
            else -> {
                runs(body, board)
                recipes(body, board)
                body.add(context.pill("Gym connection", "gym-connection") { configuring = true; world.send(json("action" to "gym_view")) }, 14, -2)
            }
        }
        board.objectOrNull("launch")?.let { launch(body, it, board.optBoolean("active")) }
        val notices = board.optJSONArray("notices").strings()
        require(notices.size <= 16) { "The Gym board exceeds its notice limit." }
        for (notice in notices) body.add(context.label(notice, 12f, selectable = true), 6)
        return body
    }

    private fun connection(body: LinearLayout, board: JSONObject) {
        body.add(context.label("Connect a Gym host", 17f, bold = true), 12)
        body.add(context.label("Create a Gym connection on your host for this world key, then paste its gym-connect: code."), 6)
        val publicKey = board.getString("public_key")
        body.add(context.label(publicKey, 11f, mono = true, key = "gym-public-key", selectable = true), 8)
        body.add(context.pill("Copy public key", "gym-copy-key") { Clipboard.copy(context, "Gym public key", publicKey) }, 8, -2)
        val input = EditText(context).apply {
            hint = "Gym connection code"; tag = "gym-code"; contentDescription = "Gym connection code"
            setText(codeDraft); minLines = 3; maxLines = 6; textSize = 13f
            setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
            background = context.rounded(Palette.RAISED, 10f); setPadding(context.dp(10), context.dp(8), context.dp(10), context.dp(8))
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            isSaveEnabled = false; importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            filters = arrayOf(InputFilter.LengthFilter(65_536))
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) { codeDraft = s.toString() }
                override fun afterTextChanged(s: Editable?) = Unit
            })
        }
        body.add(input, 10)
        body.add(context.pill("Connect Gym", "gym-connect", primary = true) {
            codeDraft = input.text.toString()
            if (codeDraft.isNotBlank() && world.configureGym(codeDraft)) { codeDraft = ""; configuring = false; world.send(json("action" to "gym_view")) }
        }, 10, -2)
        body.add(context.label("This connection is separate from your computers' access. Only runs the host lists can start, and only after you confirm.",
            12f, Palette.SECONDARY), 8)
        if (board.optBoolean("configured")) body.add(context.pill("Back to board", "gym-back") {
            configuring = false; world.send(json("action" to "gym_view"))
        }, 8, -2)
    }

    /** True while the connection form shows, so a new revision keeps the typed code. */
    fun editing(board: JSONObject?) = board != null && (!board.optBoolean("configured") || configuring)

    private fun runs(body: LinearLayout, board: JSONObject) {
        body.add(context.label("Microcoder and Terminal-Bench runs", 16f, bold = true), 12)
        val runs = board.getJSONArray("runs").objects()
        require(runs.size <= 64) { "The Gym board exceeds its run limit." }
        if (runs.isEmpty()) body.add(context.label("No runs are available in this snapshot."), 6)
        for (run in runs.sortedBy { rank(it.optString("category")) }) {
            val row = context.column().apply {
                tag = "gym-run-${run.getString("id")}"
                isClickable = true; isFocusable = true
                setPadding(0, context.dp(8), 0, context.dp(8))
                setOnClickListener { world.send(json("action" to "gym_select_run", "id" to run.getString("id"))) }
                add(context.label(run.getString("title"), 15f, bold = true))
                add(context.label("${run.getString("category")} · ${run.getString("status")}", 12f, Palette.SECONDARY), 2)
                progress(this, run)
                add(context.label(summary(run), 12f), 2)
            }
            body.add(row, 4)
            body.addDivider(4)
        }
    }

    private fun recipes(body: LinearLayout, board: JSONObject) {
        body.add(context.label("Supported new runs", 16f, bold = true), 14)
        val recipes = board.getJSONArray("recipes").objects()
        require(recipes.size <= 16) { "The Gym board exceeds its recipe limit." }
        if (recipes.isEmpty()) body.add(context.label("This connection has no supported start recipes."), 6)
        for (recipe in recipes) body.add(context.pill(recipe.getString("title"), "gym-recipe-${recipe.getString("id")}") {
            world.send(json("action" to "gym_select_recipe", "id" to recipe.getString("id")))
        }.enabled(board.optBoolean("active") && !board.optBoolean("stale")), 8, -2)
    }

    private fun details(body: LinearLayout, run: JSONObject) {
        body.add(context.label(run.getString("title"), 17f, bold = true, key = "gym-run-title"), 12)
        body.add(context.label("${run.getString("category")} · ${run.getString("status")}"), 4)
        progress(body, run)
        body.add(context.label(summary(run)), 4)
        val metrics = run.getJSONArray("metrics").objects()
        require(metrics.size <= 4) { "The Gym run exceeds its metric limit." }
        metrics.forEachIndexed { index, metric ->
            val points = metric.getJSONArray("points").objects()
            require(points.size <= 64) { "The Gym metric exceeds its point limit." }
            val samples = points.map { it.getLong("step").toDouble() to it.getDouble("value").also { v -> require(v.isFinite()) } }
            val unit = metric.getString("unit")
            body.add(context.label("${metric.getString("name")} ($unit)", 15f, bold = true), 12)
            if (samples.isEmpty()) { body.add(context.label("No recorded points."), 4); return@forEachIndexed }
            body.addView(MetricChart(context, samples).apply {
                contentDescription = "${metric.getString("name")}, ${samples.size} recorded points"
            }, LinearLayout.LayoutParams(-1, context.dp(160)).apply { topMargin = context.dp(6) })
            val values = context.column().apply { visibility = View.GONE }
            body.add(context.pill("Recorded values", "gym-metric-$index-values") {
                values.visibility = if (values.visibility == View.GONE) View.VISIBLE else View.GONE
            }, 6, -2)
            samples.forEach { (step, value) -> values.add(context.label("Step ${step.toLong()}: $value $unit", 12f, selectable = true)) }
            body.add(values)
        }
        body.add(context.label("Source: ${run.getString("source")}", 12f, selectable = true), 12)
        body.add(context.label(run.getString("provenance"), 12f, Palette.SECONDARY, selectable = true), 4)
        body.add(context.label("Completed means the run finished, not that it passed.",
            12f, Palette.SECONDARY), 4)
    }

    private fun recipe(body: LinearLayout, recipe: JSONObject, board: JSONObject) {
        val budget = recipe.getJSONObject("budget")
        body.add(context.label(recipe.getString("title"), 17f, bold = true), 12)
        body.add(context.label(recipe.getString("detail"), selectable = true), 6)
        body.add(context.label("Time limit: ${budget.getLong("wall_ms") / 1000} seconds · Maximum starts: ${budget.getInt("max_starts")}"), 6)
        body.add(context.label(if (budget.optBoolean("spend_enforced")) "The host enforces this recipe's spending limit."
            else "No dollar limit is enforced for this recipe."), 4)
        body.add(context.label("Recipe revision: ${recipe.getString("revision")}", 11f, mono = true, selectable = true), 4)
        body.add(context.label("Starting sends this recipe to the host. Leaving the Gym does not cancel the run."), 6)
        val phase = board.objectOrNull("launch")?.optString("phase")
        body.add(context.pill("Start this run", "gym-confirm-launch", primary = true) { world.send(json("action" to "gym_launch")) }
            .enabled(board.optBoolean("active") && !board.optBoolean("stale") && phase !in listOf("sending", "unknown")), 10, -2)
    }

    private fun launch(body: LinearLayout, launch: JSONObject, active: Boolean) {
        body.add(context.label("Request: ${launch.getString("phase")}", 15f, bold = true, key = "gym-launch-status"), 14)
        body.add(context.label(launch.getString("request_id"), 11f, mono = true, selectable = true), 2)
        launch.objectOrNull("receipt")?.let {
            body.add(context.label("Host receipt: ${it.getString("status")} · Run ${it.getString("run_id")}", 12f), 4)
        }
        launch.textOrNull("error")?.let { body.add(context.label(it, 12f), 4) }
        if (launch.optString("phase") == "sending") body.add(context.label("Waiting for the host receipt…", 12f, Palette.SECONDARY), 4)
        if (launch.optString("phase") == "unknown") {
            body.add(context.label("We couldn't confirm the host got this. Retrying won't start it twice.", 12f), 4)
            body.add(context.pill("Retry the same request", "gym-retry") { world.send(json("action" to "gym_retry")) }.enabled(active), 6, -2)
        }
    }

    private fun progress(body: LinearLayout, run: JSONObject) {
        if (run.isNull("completed") || run.isNull("total") || !run.has("completed")) { body.add(context.label("Progress unavailable", 12f), 2); return }
        val total = run.getLong("total"); val done = run.getLong("completed")
        body.add(context.label("$done / $total recorded", 12f), 2)
        if (total > 0) body.add(ProgressBar(context, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 1000; progress = (done.toDouble() / total * 1000).toInt().coerceIn(0, 1000)
            progressTintList = android.content.res.ColorStateList.valueOf(Palette.PRIMARY)
        }, 2)
    }

    private fun rank(category: String) = when (category) { "agent" -> 0; "evaluation" -> 1; else -> 2 }

    private fun summary(run: JSONObject): String {
        val cost = if (run.isNull("cost_usd") || !run.has("cost_usd")) "Cost unavailable" else String.format(Locale.US, "$%.4f", run.getDouble("cost_usd"))
        val time = if (run.isNull("elapsed_ms") || !run.has("elapsed_ms")) "Time unavailable" else "${run.getLong("elapsed_ms") / 1000} s"
        return "$cost · $time"
    }
}

/** A metric's recorded points as a white line with axis-free marks. */
private class MetricChart(context: Context, private val points: List<Pair<Double, Double>>) : View(context) {
    private val ink = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.PRIMARY; strokeWidth = context.dp(2).toFloat() }
    private val grid = Paint().apply { color = 0x33FFFFFF; strokeWidth = 1f }
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        for (i in 0..4) { val y = 8 + (height - 16) * i / 4f; canvas.drawLine(8f, y, width - 8f, y, grid) }
        if (points.isEmpty()) return
        val minX = points.minOf { it.first }; val spanX = (points.maxOf { it.first } - minX).coerceAtLeast(1.0)
        val minY = points.minOf { it.second }; val spanY = (points.maxOf { it.second } - minY).coerceAtLeast(0.000001)
        val mapped = points.map { ((it.first - minX) / spanX * (width - 16) + 8).toFloat() to
            (height - 8 - (it.second - minY) / spanY * (height - 16)).toFloat() }
        mapped.zipWithNext().forEach { (a, b) -> canvas.drawLine(a.first, a.second, b.first, b.second, ink) }
        mapped.forEach { canvas.drawCircle(it.first, it.second, 4f, ink) }
    }
}
