package com.openagents.coder

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.view.View
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import org.json.JSONObject
import java.util.Locale

/** Native presentation of the Rust-owned board and explicitly confirmed recipes. */
class GymPanel(private val context: Context, private val world: VerseSurface) {
    private var configuring = false
    private var codeDraft = ""
    private var connectionView: View? = null
    fun build(board: JSONObject): View {
        val connection = !board.optBoolean("configured") || configuring
        if (connection) connectionView?.let { existing ->
            existing.findViewWithTag<android.widget.TextView>("gym-status")?.text = board.optString("status")
            refreshError(existing, board)
            return existing
        }
        val view = context.column().apply {
        addView(context.label(board.optString("status"), "gym-status"))
        if (board.optBoolean("stale")) addView(context.label("Snapshot is stale. New starts are unavailable."))
        addView(context.label("", "gym-error"))
        refreshError(this, board)
        val body = context.column()
        addView(ScrollView(context).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
        when {
            !board.optBoolean("configured") || configuring -> connection(body, board)
            board.optJSONObject("selected_run") != null -> {
                body.addView(context.button("All runs", "gym-all-runs") { world.send(json("action" to "gym_close_detail")) })
                details(body, board.getJSONObject("selected_run"))
            }
            board.optJSONObject("selected_recipe") != null -> {
                body.addView(context.button("All runs", "gym-all-runs") { world.send(json("action" to "gym_close_detail")) })
                recipe(body, board.getJSONObject("selected_recipe"), board)
            }
            else -> {
                body.addView(context.label("Microcoder and Terminal-Bench runs", size = 18f))
                val runs = board.getJSONArray("runs")
                require(runs.length() <= 64) { "The Gym board exceeds its run limit." }
                if (runs.length() == 0) body.addView(context.label("No runs are available in this snapshot."))
                for (index in 0 until runs.length()) {
                    val run = runs.getJSONObject(index)
                    body.addView(context.button("${run.getString("title")}\n${run.getString("category")} · ${run.getString("status")}\n${summary(run)}",
                        "gym-run-${run.getString("id")}") { world.send(json("action" to "gym_select_run", "id" to run.getString("id"))) })
                    progress(body, run)
                }
                body.addView(context.label("Supported new runs", size = 18f))
                val recipes = board.getJSONArray("recipes")
                require(recipes.length() <= 16)
                if (recipes.length() == 0) body.addView(context.label("This connection has no supported start recipes."))
                for (index in 0 until recipes.length()) {
                    val recipe = recipes.getJSONObject(index)
                    body.addView(context.button(recipe.getString("title"), "gym-recipe-${recipe.getString("id")}") {
                        world.send(json("action" to "gym_select_recipe", "id" to recipe.getString("id")))
                    }.apply { isEnabled = board.optBoolean("active") && !board.optBoolean("stale") })
                }
                body.addView(context.button("Gym connection", "gym-connection") {
                    configuring = true; world.send(json("action" to "gym_view"))
                })
            }
        }
        board.optJSONObject("launch")?.let { launch ->
            body.addView(context.label("Request: ${launch.getString("phase")}", "gym-launch-status"))
            body.addView(context.label(launch.getString("request_id")))
            launch.optJSONObject("receipt")?.let {
                body.addView(context.label("Host receipt: ${it.getString("status")} · Run ${it.getString("run_id")}"))
            }
            launch.textOrNull("error")?.let { body.addView(context.label(it)) }
            if (launch.optString("phase") == "unknown") {
                body.addView(context.label("The host may already have accepted this request. Retry uses the same request identity."))
                body.addView(context.button("Retry the same request", "gym-retry") { world.send(json("action" to "gym_retry")) }
                    .apply { isEnabled = board.optBoolean("active") })
            }
        }
        val notices = board.getJSONArray("notices")
        require(notices.length() <= 16)
        for (index in 0 until notices.length()) body.addView(context.label(notices.getString(index)))
        }
        connectionView = if (connection) view else null
        return view
    }

    fun refreshError(container: View, board: JSONObject?) {
        val problem = board?.textOrNull("error") ?: world.snapshot?.textOrNull("error")
            ?: world.gymStorageError ?: world.nativeError
        container.findViewWithTag<android.widget.TextView>("gym-error")?.apply {
            text = problem?.let { if (it.length <= 2048) it else it.take(2048) + "…" }.orEmpty()
            visibility = if (problem == null) View.GONE else View.VISIBLE
        }
    }

    private fun connection(body: LinearLayout, board: JSONObject) {
        body.addView(context.label("Connect a Gym host", size = 18f))
        body.addView(context.label("Create a Gym connection grant on your host for this device public key, then paste its gym-connect: code."))
        val publicKey = board.getString("public_key")
        body.addView(context.label(publicKey, "gym-public-key", 11f))
        body.addView(context.button("Copy public key", "gym-copy-key") {
            context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Gym public key", publicKey))
        })
        val input = EditText(context).apply {
            hint = "Gym connection code"; tag = "gym-code"; setTextColor(AMBER); setText(codeDraft); minLines = 3
            isSaveEnabled = false; importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
            filters = arrayOf(android.text.InputFilter.LengthFilter(65_536))
        }
        input.addTextChangedListener(object : android.text.TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) { codeDraft = s.toString() }
            override fun afterTextChanged(s: android.text.Editable?) = Unit
        })
        body.addView(input)
        body.addView(context.button("Connect Gym", "gym-connect") {
            codeDraft = input.text.toString()
            if (world.configureGym(codeDraft)) { codeDraft = ""; configuring = false; world.send(json("action" to "gym_view")) }
        })
        body.addView(context.label("This grant is separate from saved-chat access. Only the host's listed recipes can start, after you confirm."))
        if (board.optBoolean("configured")) body.addView(context.button("Back to board", "gym-back") {
            configuring = false; connectionView = null; world.send(json("action" to "gym_view"))
        })
    }
    private fun details(body: LinearLayout, run: JSONObject) {
        body.addView(context.label(run.getString("title"), "gym-run-title", 18f))
        body.addView(context.label("${run.getString("category")} · ${run.getString("status")}"))
        progress(body, run)
        body.addView(context.label(summary(run)))
        val metrics = run.getJSONArray("metrics")
        require(metrics.length() <= 4)
        for (index in 0 until metrics.length()) {
            val metric = metrics.getJSONObject(index)
            val points = metric.getJSONArray("points")
            require(points.length() <= 64)
            val samples = (0 until points.length()).map { points.getJSONObject(it) }.map {
                it.getLong("step").toDouble() to it.getDouble("value").also { value -> require(value.isFinite()) }
            }
            body.addView(context.label("${metric.getString("name")} (${metric.getString("unit")})", size = 18f))
            if (samples.isEmpty()) body.addView(context.label("No recorded points.")) else {
                body.addView(MetricChart(context, samples).apply {
                    contentDescription = "${metric.getString("name")}, ${samples.size} recorded points"
                }, LinearLayout.LayoutParams(-1, context.dp(160)))
                val values = context.column().apply { visibility = View.GONE }
                body.addView(context.button("Recorded values", "gym-metric-$index-values") {
                    values.visibility = if (values.visibility == View.GONE) View.VISIBLE else View.GONE
                })
                samples.forEach { (step, value) -> values.addView(context.label("Step ${step.toLong()}: $value ${metric.getString("unit")}")) }
                body.addView(values)
            }
        }
        body.addView(context.label("Source: ${run.getString("source")}"))
        body.addView(context.label(run.getString("provenance")))
        body.addView(context.label("Completed describes the recorded process; it does not by itself establish benchmark success."))
    }
    private fun recipe(body: LinearLayout, recipe: JSONObject, board: JSONObject) {
        val budget = recipe.getJSONObject("budget")
        body.addView(context.label(recipe.getString("title"), size = 18f))
        body.addView(context.label(recipe.getString("detail")))
        body.addView(context.label("Time limit: ${budget.getLong("wall_ms") / 1000} seconds · Maximum starts: ${budget.getInt("max_starts")}"))
        body.addView(context.label(if (budget.optBoolean("spend_enforced")) "The host enforces this recipe's spending limit." else "No dollar limit is enforced for this recipe."))
        if (!budget.isNull("spend_limit_usd")) body.addView(context.label("Spending limit: $${budget.getDouble("spend_limit_usd")}"))
        body.addView(context.label("Recipe revision: ${recipe.getString("revision")}", size = 11f))
        body.addView(context.label("Starting submits this exact recipe to the host. Leaving the Gym does not cancel the run."))
        body.addView(context.button("Start this run", "gym-confirm-launch") { world.send(json("action" to "gym_launch")) }.apply {
            isEnabled = board.optBoolean("active") && !board.optBoolean("stale") &&
                board.optJSONObject("launch")?.optString("phase") !in listOf("sending", "unknown")
        })
    }
    private fun progress(body: LinearLayout, run: JSONObject) {
        if (run.isNull("completed") || run.isNull("total")) { body.addView(context.label("Progress unavailable")); return }
        val total = run.getLong("total"); val done = run.getLong("completed")
        body.addView(context.label("$done / $total recorded"))
        if (total > 0) body.addView(ProgressBar(context, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 1000; progress = (done.toDouble() / total * 1000).toInt().coerceIn(0, 1000)
        })
    }
    private fun summary(run: JSONObject): String {
        val cost = if (run.isNull("cost_usd")) "Cost unavailable" else String.format(Locale.US, "$%.4f", run.getDouble("cost_usd"))
        val time = if (run.isNull("elapsed_ms")) "Time unavailable" else "${run.getLong("elapsed_ms") / 1000} s"
        return "$cost · $time"
    }
}

private class MetricChart(context: Context, private val points: List<Pair<Double, Double>>) : View(context) {
    private val ink = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = AMBER; strokeWidth = context.dp(2).toFloat() }
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        if (points.isEmpty()) return
        val minX = points.minOf { it.first }; val spanX = (points.maxOf { it.first } - minX).coerceAtLeast(1.0)
        val minY = points.minOf { it.second }; val spanY = (points.maxOf { it.second } - minY).coerceAtLeast(0.000001)
        val mapped = points.map { ((it.first - minX) / spanX * (width - 16) + 8).toFloat() to
            (height - 8 - (it.second - minY) / spanY * (height - 16)).toFloat() }
        mapped.zipWithNext().forEach { (a, b) -> canvas.drawLine(a.first, a.second, b.first, b.second, ink) }
        mapped.forEach { canvas.drawCircle(it.first, it.second, 4f, ink) }
    }
}
