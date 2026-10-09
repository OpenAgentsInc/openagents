// The Gym in chat (wireframe revision 3): the main menu, the first run, the
// cards a chat reply carries, and the sheets over them. Rust builds every
// value with the app's own words and mints every button's ID; this host
// draws them in the menu's black-and-white style and sends back only the ID
// of the button tapped (`gym`). No number here is made up: a value Rust
// hasn't read is absent, and the host draws a gray bar instead.
package com.openagents.app

import android.app.Dialog
import android.content.Context
import android.content.Intent
import android.view.Gravity
import android.view.View
import android.view.Window
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONArray
import org.json.JSONObject

internal class GymViews(private val context: Context, private val tap: (String) -> Unit) {
    private var dialog: Dialog? = null
    private var shownSheet: String? = null

    private val card = 0xFF121212.toInt()

    // Building blocks

    private fun title(value: String, size: Float, color: Int = Palette.PRIMARY) =
        context.text(value.uppercase(), size, color).apply {
            typeface = Fonts.typeface(context, Fonts.BOLD)
            letterSpacing = 0.03f
        }

    private fun body(value: String, tone: String = "body") =
        context.text(value, 16f, if (tone == "quiet") Palette.SECONDARY else Palette.PRIMARY).apply {
            if (tone == "strong") typeface = Fonts.typeface(context, Fonts.BOLD)
        }

    private fun primary(button: JSONObject, busy: Boolean = false): TextView =
        title(button.getString("label").let { if (busy && !it.endsWith("…")) "$it …" else it }, 18f, Palette.BACKGROUND).apply {
            gravity = Gravity.CENTER
            minHeight = context.dp(56)
            val enabled = button.optBoolean("enabled", true)
            background = context.rounded(if (enabled) Palette.PRIMARY else 0xFF333333.toInt(), 14f)
            if (!enabled) setTextColor(0xFF8C8C8C.toInt())
            tag = button.getString("id")
            contentDescription = button.getString("label")
            isClickable = enabled; isFocusable = true
            setOnClickListener { if (enabled) tap(button.getString("id")) }
        }

    private fun outlined(button: JSONObject): TextView =
        context.text(button.getString("label"), 15f).apply {
            gravity = Gravity.CENTER
            minHeight = context.dp(44)
            setPadding(context.dp(14), 0, context.dp(14), 0)
            background = context.rounded(0, 22f, Palette.BORDER)
            tag = button.getString("id")
            isClickable = true; isFocusable = true
            setOnClickListener { tap(button.getString("id")) }
        }

    private fun chip(button: JSONObject): TextView =
        context.pill(button.getString("label"), button.getString("id")) { tap(button.getString("id")) }

    private fun mark(value: String) = context.text(when (value) {
        "check" -> "✓"; "cross" -> "✗"; "wait" -> "…"; "dot" -> "•"; else -> "–"
    }, 16f, if (value == "cross" || value == "none") Palette.TERTIARY else Palette.PRIMARY).apply {
        typeface = Fonts.typeface(context, Fonts.BOLD)
        minWidth = context.dp(22)
    }

    private fun item(item: JSONObject): View = context.row().apply {
        item.optJSONArray("marks")?.let { marks -> for (i in 0 until marks.length()) addView(mark(marks.getString(i))) }
        val text = context.column().apply {
            addView(body(item.getString("text")))
            item.textOrNull("detail")?.let { addView(context.text(it, 14f, Palette.SECONDARY)) }
        }
        addView(text, LinearLayout.LayoutParams(0, -2, 1f))
        item.textOrNull("trailing")?.let { addView(body(it, "strong")) }
    }

    private fun compare(compare: JSONObject, size: Float): View = context.row().apply {
        gravity = Gravity.BOTTOM
        compare.textOrNull("without")?.let { without ->
            addView(context.column().apply {
                addView(context.text(compare.getString("without_label"), 12f, Palette.SECONDARY))
                addView(title(without, size, Palette.SECONDARY))
            })
            addView(context.text("  →  ", size * 0.6f, Palette.SECONDARY))
        }
        addView(context.column().apply {
            addView(context.text(compare.getString("with_label"), 12f, Palette.SECONDARY))
            addView(title(compare.getString("with"), size))
        })
    }

    private fun blocks(progress: JSONObject): View = context.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        val done = progress.getInt("done"); val total = maxOf(progress.getInt("total"), 1)
        for (i in 0 until total) addView(View(context).apply {
            background = context.rounded(if (i < done) Palette.PRIMARY else Palette.RAISED, 3f, Palette.BORDER)
        }, LinearLayout.LayoutParams(context.dp(14), context.dp(14)).apply { marginEnd = context.dp(4) })
        addView(context.text("  ${progress.getString("label")}", 14f, Palette.SECONDARY))
    }

    private fun LinearLayout.gap(view: View, top: Int = 10) =
        addView(view, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(top) })

    private fun buttons(list: JSONArray?, build: (JSONObject) -> View): View? {
        val items = list?.objects().orEmpty()
        if (items.isEmpty()) return null
        return FlowLayout(context, context.dp(8)).apply { for (b in items) addView(build(b)) }
    }

    // Cards

    /** A card under a chat reply (`CARD-01` to `CARD-07`), with its chips below. */
    fun card(value: JSONObject): View {
        val outer = context.column()
        val frame = context.column().apply {
            setPadding(context.dp(16), context.dp(16), context.dp(16), context.dp(16))
            background = context.rounded(card, 16f, if (value.optBoolean("busy")) 0xFF8C8C8C.toInt() else Palette.BORDER)
        }
        frame.addView(title(value.getString("title"), if (value.getString("kind") == "result") 22f else 18f))
        value.textOrNull("badge")?.let { frame.gap(body(it, "strong"), 4) }
        value.textOrNull("step")?.let { frame.gap(title(it, 12f, Palette.TERTIARY), 4) }
        value.objectOrNull("compare")?.let { frame.gap(compare(it, 26f)) }
        value.optJSONArray("progress")?.objects()?.forEach { frame.gap(blocks(it), 8) }
        value.optJSONArray("items")?.objects()?.forEach { frame.gap(item(it), 6) }
        value.optJSONArray("lines")?.objects()?.forEach { frame.gap(body(it.getString("text"), it.getString("tone")), 8) }
        value.objectOrNull("primary")?.let { frame.gap(primary(it), 12) }
        buttons(value.optJSONArray("secondary")) { outlined(it) }?.let { frame.gap(it) }
        value.textOrNull("source")?.let { frame.gap(context.text(it, 12f, Palette.TERTIARY)) }
        outer.addView(frame, LinearLayout.LayoutParams(-1, -2))
        buttons(value.optJSONArray("chips")) { chip(it) }?.let { outer.gap(it, 8) }
        outer.tag = "gym-card-${value.getString("kind")}"
        return outer
    }

    /** A card at its own height up to half the screen, scrolling inside past it. */
    fun cappedCard(value: JSONObject): View = object : ScrollView(context) {
        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            val cap = resources.displayMetrics.heightPixels / 2
            super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(cap, MeasureSpec.AT_MOST))
        }

        // A card taller than its cap scrolls itself: keep the transcript
        // from taking the drag, or a draft's button stays out of reach.
        private fun claim() {
            if (canScrollVertically(1) || canScrollVertically(-1)) parent?.requestDisallowInterceptTouchEvent(true)
        }

        override fun onInterceptTouchEvent(event: android.view.MotionEvent): Boolean {
            if (event.actionMasked == android.view.MotionEvent.ACTION_DOWN) claim()
            return super.onInterceptTouchEvent(event)
        }

        override fun onTouchEvent(event: android.view.MotionEvent): Boolean {
            claim()
            return super.onTouchEvent(event)
        }
    }.apply { addView(card(value)); isNestedScrollingEnabled = true }

    // Sheets

    /** Shows Rust's sheet over the app, or closes it when Rust has none. */
    fun sheet(value: JSONObject?) {
        if (value == null) { shownSheet = null; dialog?.dismiss(); dialog = null; return }
        val encoded = value.toString()
        if (encoded == shownSheet) return
        shownSheet = encoded
        val content = sheetBody(value)
        val current = dialog
        if (current != null) { current.setContentView(content); return }
        dialog = Dialog(context, android.R.style.Theme_Black_NoTitleBar_Fullscreen).apply {
            requestWindowFeature(Window.FEATURE_NO_TITLE)
            setContentView(content)
            setOnCancelListener {
                // Back closed it while Rust still shows it: tell Rust.
                val close = value.objectOrNull("close")?.getString("id")
                    ?: if (value.getString("kind") == "stop") "sheet.keep" else value.objectOrNull("primary")?.getString("id")
                dialog = null; shownSheet = null
                close?.let(tap)
            }
            show()
        }
    }

    private fun sheetBody(value: JSONObject): View {
        val root = context.column().apply { setBackgroundColor(0xFF0A0A0A.toInt()) }
        val bar = context.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(20), context.dp(16), context.dp(8), context.dp(8))
            addView(title(value.getString("title"), 18f), LinearLayout.LayoutParams(0, -2, 1f))
            value.objectOrNull("close")?.let { close ->
                addView(context.text("✕", 20f).apply {
                    setPadding(context.dp(12), context.dp(8), context.dp(12), context.dp(8))
                    contentDescription = close.getString("label"); tag = close.getString("id")
                    setOnClickListener { tap(close.getString("id")) }
                })
            }
        }
        root.addView(bar)
        root.addView(context.divider(), LinearLayout.LayoutParams(-1, 1))
        val list = context.column().apply { setPadding(context.dp(20), context.dp(8), context.dp(20), context.dp(16)) }
        value.textOrNull("big")?.let { list.gap(title(it, 96f).apply { gravity = Gravity.CENTER }) }
        value.textOrNull("headline")?.let { list.gap(title(it, 30f)) }
        value.objectOrNull("compare")?.let { list.gap(compare(it, 38f), 14) }
        value.optJSONArray("sections")?.objects()?.forEach { section ->
            section.textOrNull("heading")?.let { heading ->
                val loud = heading.uppercase() == heading
                list.gap(if (loud) title(heading, 13f, Palette.TERTIARY) else body(heading, "strong"), 18)
            }
            section.optJSONArray("items")?.objects()?.forEach { list.gap(item(it), 6) }
            section.optJSONArray("lines")?.objects()?.forEach { list.gap(body(it.getString("text"), it.getString("tone")), 8) }
        }
        val footerChoices = value.getString("kind") in setOf("publish", "stop")
        if (!footerChoices) buttons(value.optJSONArray("secondary")) { outlined(it) }?.let { list.gap(it, 14) }
        value.objectOrNull("bar")?.let { level ->
            list.gap(xpBar(level.getInt("value"), level.getInt("max")), 16)
            list.gap(context.text(level.getString("label"), 12f, Palette.SECONDARY), 4)
        }
        root.addView(ScrollView(context).apply { addView(list) }, LinearLayout.LayoutParams(-1, 0, 1f))
        val foot = context.column().apply { setPadding(context.dp(20), context.dp(8), context.dp(20), context.dp(20)) }
        value.textOrNull("next")?.let { foot.gap(body(it, "quiet"), 0) }
        value.objectOrNull("primary")?.let { foot.gap(primary(it, value.optBoolean("busy"))) }
        if (footerChoices) value.optJSONArray("secondary")?.objects()?.forEach { button ->
            foot.gap(context.text(button.getString("label"), 16f, Palette.SECONDARY).apply {
                gravity = Gravity.CENTER; minHeight = context.dp(44)
                tag = button.getString("id")
                setOnClickListener { tap(button.getString("id")) }
            }, 4)
        }
        root.addView(foot)
        // The dialog draws edge to edge: keep its buttons above the
        // navigation bar, as the tabs do.
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            view.setPadding(bars.left, 0, bars.right, bars.bottom)
            insets
        }
        return root
    }

    /**
     * The fill's share of the bar as a layout weight, so the first layout
     * pass draws it. (Sizing it after layout left it empty: the menu is
     * rebuilt before a posted resize lands.)
     */
    private fun xpBar(value: Int, max: Int): View = context.row().apply {
        background = context.rounded(Palette.RAISED, 4f, Palette.BORDER)
        val share = (value.toFloat() / maxOf(max, 1)).coerceIn(0.03f, 1f)
        addView(View(context).apply { background = context.rounded(Palette.PRIMARY, 4f) },
            LinearLayout.LayoutParams(0, context.dp(8), share))
        if (share < 1f) addView(View(context), LinearLayout.LayoutParams(0, context.dp(8), 1f - share))
    }

    /** Opens the system share sheet with Rust's text. */
    fun share(text: String) {
        context.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"; putExtra(Intent.EXTRA_TEXT, text)
        }, null))
    }

    // The menu and the first run

    /** `SCR-01` Main menu. */
    fun menu(value: JSONObject): View {
        val list = context.column().apply { setPadding(context.dp(20), context.dp(12), context.dp(20), context.dp(20)) }
        list.addView(title("OPENAGENTS", 20f).apply { letterSpacing = 0.15f; contentDescription = "OpenAgents" })
        val player = value.getJSONObject("player")
        list.gap(context.column().apply {
            setPadding(context.dp(14), context.dp(12), context.dp(14), context.dp(12))
            background = context.rounded(0xFF0E0E0E.toInt(), 16f, Palette.BORDER)
            addView(body(player.getString("name"), "strong"))
            if (player.has("level") && !player.isNull("level")) {
                gap(title("LEVEL ${player.getInt("level")}", 13f), 6)
                gap(xpBar(player.getInt("bar_value"), player.getInt("bar_max")), 6)
                player.textOrNull("xp_label")?.let { gap(context.text(it, 12f, Palette.SECONDARY), 4) }
            } else {
                // Still reading: a gray bar, never a 0.
                gap(View(context).apply { background = context.rounded(Palette.RAISED, 4f) }, 8)
            }
            tag = "menu-player"
        }, 14)
        list.gap(context.text("● ${value.getString("status")}", 13f).apply {
            setPadding(context.dp(12), context.dp(6), context.dp(12), context.dp(6))
            background = context.rounded(Palette.SURFACE, 14f, Palette.BORDER)
        }.let { pill -> FrameLayout(context).apply { addView(pill, FrameLayout.LayoutParams(-2, -2)) } }, 14)
        list.gap(body(value.getString("next"), "quiet").apply { tag = "menu-next" }, 14)
        val primary = value.getJSONObject("primary")
        list.gap(context.column().apply {
            setPadding(context.dp(16), context.dp(14), context.dp(16), context.dp(14))
            background = context.rounded(Palette.PRIMARY, 14f)
            addView(title(primary.getString("label"), 20f, Palette.BACKGROUND))
            addView(context.text(value.getString("primary_subtitle"), 14f, 0xFF5A5A5A.toInt()))
            tag = primary.getString("id"); contentDescription = primary.getString("label")
            isClickable = true; isFocusable = true
            setOnClickListener { tap(primary.getString("id")) }
        }, 12)
        buttons(value.optJSONArray("chips")) { chip(it) }?.let { list.gap(it, 12) }
        value.optJSONArray("rows")?.objects()?.forEach { row ->
            val button = row.getJSONObject("button")
            list.gap(context.column().apply {
                setPadding(context.dp(16), context.dp(14), context.dp(16), context.dp(14))
                background = context.rounded(0xFF0E0E0E.toInt(), 14f, Palette.BORDER)
                addView(title(button.getString("label"), 20f))
                addView(context.text(row.getString("subtitle"), 14f, Palette.SECONDARY))
                tag = button.getString("id"); contentDescription = button.getString("label")
                isClickable = true; isFocusable = true
                setOnClickListener { tap(button.getString("id")) }
            }, 10)
        }
        list.gap(context.text(value.getString("footer"), 12f, Palette.TERTIARY), 16)
        return ScrollView(context).apply { setBackgroundColor(Palette.BACKGROUND); addView(list) }
    }

    /** `SCR-02` Choose your agent, and the intro's end card. */
    fun firstRun(value: JSONObject): View {
        val root = context.column().apply {
            setPadding(context.dp(20), context.dp(16), context.dp(20), context.dp(20))
            setBackgroundColor(Palette.BACKGROUND)
        }
        value.textOrNull("indicator")?.let { root.addView(title(it, 13f, Palette.SECONDARY)) }
        root.addView(View(context), LinearLayout.LayoutParams(-1, 0, 1f))
        root.gap(context.text(value.getString("title"), 26f).apply { typeface = Fonts.typeface(context, Fonts.BOLD) })
        value.optJSONArray("lines")?.let { lines -> for (i in 0 until lines.length()) root.gap(body(lines.getString(i), "quiet"), 6) }
        value.optJSONArray("agent")?.takeIf { it.length() == 2 }?.let { agent ->
            root.gap(context.column().apply {
                setPadding(context.dp(16), context.dp(16), context.dp(16), context.dp(16))
                background = context.rounded(card, 16f, 0xFF8C8C8C.toInt())
                addView(title("${agent.getString(0)}  ✓", 20f))
                addView(body(agent.getString(1), "quiet"))
            }, 16)
        }
        root.addView(View(context), LinearLayout.LayoutParams(-1, 0, 1f))
        root.gap(body(value.getString("next"), "quiet"))
        root.gap(primary(value.getJSONObject("primary")), 10)
        value.objectOrNull("secondary")?.let { secondary ->
            root.gap(context.text(secondary.getString("label"), 16f, Palette.SECONDARY).apply {
                gravity = Gravity.CENTER; minHeight = context.dp(44)
                tag = secondary.getString("id")
                setOnClickListener { tap(secondary.getString("id")) }
            }, 6)
        }
        return root
    }

    /** Every Gym button ID on screen, for debug launch scripts. */
    fun buttonIds(gym: JSONObject): List<String> {
        val ids = mutableListOf<String>()
        fun add(button: JSONObject?) { button?.optString("id")?.takeIf { it.isNotEmpty() }?.let(ids::add) }
        fun addAll(list: JSONArray?) { list?.objects()?.forEach { add(it) } }
        val sheet = gym.objectOrNull("sheet")
        if (sheet != null) {
            add(sheet.objectOrNull("primary")); add(sheet.objectOrNull("close")); addAll(sheet.optJSONArray("secondary"))
            return ids
        }
        when (gym.optString("screen")) {
            "menu" -> gym.objectOrNull("menu")?.let { menu ->
                add(menu.objectOrNull("primary")); addAll(menu.optJSONArray("chips"))
                menu.optJSONArray("rows")?.objects()?.forEach { add(it.objectOrNull("button")) }
            }
            "first_run" -> gym.objectOrNull("first_run")?.let { add(it.objectOrNull("primary")); add(it.objectOrNull("secondary")) }
            else -> gym.objectOrNull("cards")?.let { cards ->
                cards.keys().asSequence().sorted().forEach { key ->
                    val card = cards.getJSONObject(key)
                    add(card.objectOrNull("primary")); addAll(card.optJSONArray("secondary")); addAll(card.optJSONArray("chips"))
                }
            }
        }
        return ids
    }
}
