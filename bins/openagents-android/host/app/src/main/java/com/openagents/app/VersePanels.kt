// The Gym and RESULTS boards' panels over the Verse world, each with a
// leader line from the board it belongs to, as in the iOS Verse tab. A frame
// carries only a board's revision; the panel asks Rust for the board when
// the revision changes while it is open. Touches on a panel never reach the
// world.
package com.openagents.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import androidx.core.view.ViewCompat
import androidx.core.view.accessibility.AccessibilityViewCommand
import org.json.JSONObject

class VersePanels(private val context: Context, private val world: VerseSurface) {
    private val main = Handler(Looper.getMainLooper())
    val root = FrameLayout(context).apply { visibility = View.GONE }
    private val line = LeaderLine(context)
    private val panel = CappedColumn(context).apply {
        setPadding(context.dp(14), context.dp(10), context.dp(14), context.dp(14))
        background = context.rounded(0xF70A0A0A.toInt(), 16f, 0x8CFFFFFF.toInt())
        isClickable = true // Touches on the panel stay off the world.
    }
    private val headerBack = context.text("‹", 26f).apply {
        gravity = Gravity.CENTER; contentDescription = "Back"; tag = "results-back"
        setOnClickListener { world.results(json("do" to "back")) }
    }
    private val title = context.label("", 17f, bold = true)
    private val close = context.text("✕", 18f).apply { gravity = Gravity.CENTER; contentDescription = "Back to world" }
    private val scroll = ScrollView(context).apply { isFillViewport = false }
    private val fixed = context.column()
    private val gym = GymPanel(context, world)
    private val results = ResultsPanel(context, world) { mounted = null; refresh() }
    private var insetTop = 0

    private var open = "" // "gym", "results", or ""
    private var gymBoard: JSONObject? = null
    private var gymRequested = -1L
    private var resultsView: JSONObject? = null
    private var resultsRequested = -1L
    /** What the panel shows now, to rebuild only when it changes. */
    private var mounted: String? = null
    private var gymAccessible = false
    private var resultsAccessible = false
    private var accessibilityActions = listOf<Int>()

    init {
        root.addView(line, FrameLayout.LayoutParams(-1, -1))
        val header = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
        header.addView(headerBack, LinearLayout.LayoutParams(context.dp(40), context.dp(44)))
        header.addView(title, LinearLayout.LayoutParams(0, -2, 1f))
        header.addView(close, LinearLayout.LayoutParams(context.dp(44), context.dp(44)))
        panel.addView(header)
        panel.addView(fixed)
        panel.addView(scroll, LinearLayout.LayoutParams(-1, -2))
        root.addView(panel, FrameLayout.LayoutParams(-1, -1))
        close.setOnClickListener { world.send(json("action" to if (open == "gym") "close_gym" else "close_results")) }
        root.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> place() }
    }

    val showing get() = open.isNotEmpty()
    /** The open panel, `gym` or `results`, or empty. */
    val openPanel get() = open

    /** The system Back gesture: a results screen's back, else back to the world. */
    fun back() {
        if (open == "results" && resultsView?.optBoolean("can_back") == true) world.results(json("do" to "back"))
        else world.send(json("action" to if (open == "gym") "close_gym" else "close_results"))
    }

    fun setTopInset(value: Int) { if (insetTop != value) { insetTop = value; place() } }

    /** Applies a world packet. */
    fun update(packet: JSONObject?) {
        packet ?: return
        val gymOpen = packet.optBoolean("gym_open")
        val resultsOpen = packet.optBoolean("results_open")
        val next = if (gymOpen) "gym" else if (resultsOpen) "results" else ""
        val location = packet.objectOrNull("gym")
        val inside = packet.optBoolean("gym_active") && location?.optBoolean("inside") == true
        if (!inside) { gymBoard = null; gymRequested = -1 }
        packet.objectOrNull("gym_board")?.let { gymBoard = it; gymRequested = it.optLong("revision") }
        if (inside && gymOpen) {
            val revision = packet.optLong("gym_revision", -1)
            if (revision >= 0 && gymBoard?.optLong("revision") != revision && gymRequested != revision) {
                gymRequested = revision
                main.post { if (open == "gym") world.send(json("action" to "gym_view")) }
            }
        }
        val resultsActive = packet.optBoolean("results_active") && resultsOpen
        if (!resultsActive) { resultsView = null; resultsRequested = -1 }
        else packet.objectOrNull("results_view")?.let { resultsView = it; resultsRequested = it.optLong("revision") }
        if (resultsActive) {
            val revision = packet.optLong("results_revision", -1)
            if (revision >= 0 && resultsView?.optLong("revision") != revision && resultsRequested != revision) {
                resultsRequested = revision
                main.post { if (open == "results") world.send(json("action" to "results_view")) }
            }
        }
        if (next != open) {
            open = next; mounted = null
            if (next != "results") results.watching = false
            root.visibility = if (next.isEmpty()) View.GONE else View.VISIBLE
        }
        if (open.isNotEmpty()) {
            val anchor = packet.objectOrNull(if (open == "gym") "gym" else "results")
            if (anchor != null) line.anchor(anchor.optDouble("screen_x", 0.5), anchor.optDouble("screen_y", 0.5))
            refresh()
        }
        updateAccessibility(packet)
    }

    private fun refresh() {
        if (open.isEmpty()) return
        if (open == "gym") {
            title.text = "Gym"; close.tag = "gym-close"; headerBack.visibility = View.GONE
            val board = gymBoard
            // A new board revision rebuilds the panel. While a connection
            // code is being typed, only a new error or state does, so the
            // field keeps its focus.
            val problem = board?.textOrNull("error") ?: world.snapshot?.textOrNull("error") ?: world.gymStorageError
            val key = if (gym.editing(board)) "gym:edit:${board?.optBoolean("configured")}:$problem"
                else "gym:${board?.toString()?.hashCode()}:$problem"
            if (key == mounted) return
            mount(key) { gym.build(board) }
        } else {
            title.text = "Results"; close.tag = "results-close"
            val view = resultsView
            headerBack.visibility = if (view?.optBoolean("can_back") == true) View.VISIBLE else View.INVISIBLE
            if (results.scrubbing) return
            // A new screen starts at its top; a new revision of the same
            // screen keeps the reading position.
            val page = view?.objectOrNull("page")
            val screen = "results/${page?.optString("screen")}/${page?.optString("id", page.optString("attempt"))}"
            val key = "$screen:${view?.toString()?.hashCode()}:${results.watching}"
            if (key == mounted) return
            mount(key) { results.build(view, fixed, scroll) }
        }
    }

    private fun mount(key: String, build: () -> View) {
        val sameScreen = mounted?.substringBefore(':') == key.substringBefore(':')
        val y = scroll.scrollY
        mounted = key
        fixed.removeAllViews()
        // Restore the reading position first; a trace's Agent tab then
        // moves to the playhead's row in its own later post.
        if (sameScreen) scroll.post { scroll.scrollTo(0, y) }
        val content = try { build() } catch (failure: Exception) {
            context.label(failure.message ?: "This panel couldn't be shown.", 14f, Palette.SECONDARY)
        }
        scroll.removeAllViews()
        scroll.addView(content, FrameLayout.LayoutParams(-1, -2))
    }

    /** Lays the panel out below the status bar, at most 540 dp wide, near its board. */
    private fun place() {
        val width = root.width; val height = root.height
        if (width == 0 || height == 0) return
        val margin = context.dp(12)
        val panelWidth = minOf(width - 2 * margin, context.dp(540))
        val anchorX = (line.x * width).toFloat()
        val left = (anchorX - panelWidth / 2).toInt().coerceIn(margin, maxOf(margin, width - margin - panelWidth))
        val params = panel.layoutParams as FrameLayout.LayoutParams
        val top = insetTop + margin
        val panelHeight = maxOf(context.dp(80), height - top - context.dp(16))
        if (params.leftMargin != left || params.topMargin != top || params.width != panelWidth || panel.cap != panelHeight) {
            // The panel is as tall as its content, up to the space below
            // the status bar, so a short one (the trace's timeline while
            // watching the replay) leaves the world in view.
            params.leftMargin = left; params.topMargin = top; params.width = panelWidth; params.height = -2
            panel.cap = panelHeight
            params.gravity = Gravity.TOP or Gravity.START
            panel.layoutParams = params
        }
        line.panel(left.toFloat(), (left + panelWidth).toFloat(), top.toFloat())
    }

    /** TalkBack opens the Gym and RESULTS boards with the same checks as a tap on them. */
    private fun updateAccessibility(packet: JSONObject) {
        val panelOpen = packet.optBoolean("gym_open") || packet.optBoolean("results_open")
        fun reachable(name: String, active: String) = packet.optBoolean(active) && packet.objectOrNull(name)?.let {
            it.optBoolean("inside") && it.optBoolean("near") && it.optBoolean("visible")
        } == true && !panelOpen
        val gymNow = reachable("gym", "gym_active")
        val resultsNow = reachable("results", "results_active")
        if (gymNow == gymAccessible && resultsNow == resultsAccessible) return
        gymAccessible = gymNow; resultsAccessible = resultsNow
        accessibilityActions.forEach { ViewCompat.removeAccessibilityAction(world, it) }
        accessibilityActions = buildList {
            if (gymNow) add(ViewCompat.addAccessibilityAction(world, "Open Gym board", AccessibilityViewCommand { _, _ ->
                world.send(json("action" to "interact_gym"))?.optBoolean("gym_open") == true }))
            if (resultsNow) add(ViewCompat.addAccessibilityAction(world, "Open results board", AccessibilityViewCommand { _, _ ->
                world.send(json("action" to "interact_results"))?.optBoolean("results_open") == true }))
        }
    }
}

/** The leader line from a board's place on screen to the top of its panel. */
private class LeaderLine(context: Context) : View(context) {
    var x = 0.5; private set
    private var y = 0.5
    private var left = 0f; private var right = 0f; private var top = 0f
    private val ink = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0x99FFFFFF.toInt(); strokeWidth = context.dpf(2f) }
    init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }
    fun anchor(x: Double, y: Double) {
        if (!x.isFinite() || !y.isFinite()) return
        val nx = x.coerceIn(0.0, 1.0); val ny = y.coerceIn(0.0, 1.0)
        if (nx != this.x || ny != this.y) { this.x = nx; this.y = ny; invalidate(); requestLayout() }
    }
    fun panel(left: Float, right: Float, top: Float) {
        if (left != this.left || right != this.right || top != this.top) { this.left = left; this.right = right; this.top = top; invalidate() }
    }
    override fun onDraw(canvas: Canvas) {
        val ax = (x * width).toFloat(); val ay = (y * height).toFloat()
        val bx = ax.coerceIn(left + context.dpf(20f), maxOf(left + context.dpf(20f), right - context.dpf(20f)))
        canvas.drawLine(ax, ay, bx, top, ink)
    }
}

/** A column no taller than `cap` pixels. */
private class CappedColumn(context: Context) : LinearLayout(context) {
    var cap = Int.MAX_VALUE
    init { orientation = VERTICAL }
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val limit = minOf(cap, MeasureSpec.getSize(heightMeasureSpec).takeIf { MeasureSpec.getMode(heightMeasureSpec) != MeasureSpec.UNSPECIFIED } ?: cap)
        super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(limit, MeasureSpec.AT_MOST))
    }
}
