package com.openagents.app

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject

/**
 * Renders a Rust Native view (`rust-native.view.v2`) with Android widgets.
 * Controls reconcile by stable node key, so text selection, scroll position,
 * and a composer's draft survive a new revision. Callbacks return only the
 * node key; Rust resolves it against the view it issued.
 *
 * Supported: stack, list, text (every role), button, surface (a registered
 * one from `surfaces`, else a refusal),
 * and the conversation elements in [ChatViews] and [Transcript].
 *
 * `scrolling` means the host mounts this view in its own scroll view, so a
 * list lays out its rows in place instead of scrolling them.
 */
class NativeRenderer(
    private val context: Context,
    private val activate: (JSONObject, String) -> Unit,
    private val submit: ((String, String) -> Unit)? = null,
    private val scrolling: Boolean = false,
    /** Draws a surface the host registered (`gym-card:<id>`); null draws the refusal. */
    private val surfaces: ((String) -> View?)? = null,
    /**
     * The composer floats over the conversation (#11126): in the root
     * stack, what follows the transcript (the composer and anything above
     * it) is a group at the bottom, over the transcript, which scrolls under
     * it and keeps its last row clear of it.
     */
    private val floating: Boolean = false,
) {
    private class Mounted(val kind: String, val view: View) {
        var value: String? = null
        var text: TextView? = null
        var rows: LinearLayout? = null
        var transcript: RustTranscript? = null
        var composer: Composer? = null
        /** A floating root stack's transcript area and bottom group. */
        var area: FrameLayout? = null
        var group: LinearLayout? = null
        var floated: RustTranscript? = null
    }

    private val mounts = HashMap<String, Mounted>()
    private val chat = ChatViews(context, { key -> activateNode(key) }, submit)
    private var currentView: JSONObject? = null
    private var currentInstance: String? = null
    private var currentRevision = -1L
    private var seen = HashSet<String>()

    /** Shows `view` in `container`, or clears it when `view` is null. */
    fun mount(container: FrameLayout, view: JSONObject?) {
        if (view == null) { container.removeAllViews(); clear(); return }
        require(view.getString("schema") == "rust-native.view.v2") { "Unsupported Rust Native view version." }
        val instance = view.getString("instance")
        val revision = view.getLong("revision")
        if (instance == currentInstance && revision == currentRevision && container.childCount > 0) return
        if (instance != currentInstance) { container.removeAllViews(); clear() }
        // Activations resolve against the view on screen once it is applied.
        currentView = view; currentInstance = instance; currentRevision = revision
        seen = HashSet()
        val root = node(view.getJSONObject("root"), 0)
        if (container.getChildAt(0) !== root) {
            container.removeAllViews()
            (root.parent as? ViewGroup)?.removeView(root)
            container.addView(root, FrameLayout.LayoutParams(-1, if (scrolling) -2 else -1))
        }
        mounts.entries.removeAll { (key, mounted) -> (key !in seen).also { if (it) mounted.composer?.close() } }
    }

    fun clear() {
        currentView = null; currentInstance = null; currentRevision = -1
        mounts.values.forEach { it.composer?.close() }
        mounts.clear()
    }

    private fun activateNode(key: String) { currentView?.let { activate(it, key) } }

    private fun node(node: JSONObject, depth: Int): View {
        require(depth <= 16 && seen.size < 1024) { "The native view exceeds its size limit." }
        val key = node.getString("key")
        require(seen.add(key)) { "The native view repeats a key." }
        val element = node.getJSONObject("element")
        val props = element.getJSONObject("props")
        val style = node.optJSONObject("style") ?: JSONObject()
        val elementKind = element.getString("kind")
        val kind = when {
            elementKind == "text" -> "text:${props.getString("role")}"
            // A glyph button from the closed set; an unknown glyph shows the label.
            elementKind == "button" && glyph(props) != null -> props.getJSONObject("icon").let { icon ->
                when {
                    icon.optBoolean("circular") -> "button:circle"
                    icon.optBoolean("pill") -> "button:pill"
                    else -> "button:link"
                }
            }
            // A card with a context menu (`rust_native::style::Menu`).
            elementKind == "stack" && style.textOrNull("menu") == "context" &&
                props.getJSONArray("children").length() > 1 -> "stack:menu"
            elementKind == "stack" && props.getString("axis") == "wrap" -> "stack:wrap"
            floating && depth == 0 && elementKind == "stack" && props.getString("axis") == "vertical" &&
                floatSplit(props.getJSONArray("children").objects()) != null -> "stack:float"
            else -> elementKind
        }
        val mounted = mounts[key]?.takeIf { it.kind == kind } ?: create(kind, props).also { mounts[key] = it }
        val view = mounted.view
        when (kind) {
            "stack" -> (view as LinearLayout).apply {
                val horizontal = props.getString("axis") == "horizontal"
                orientation = if (horizontal) LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
                val gap = space(style.textOrNull("gap"))
                val children = props.getJSONArray("children").objects()
                replaceChildren(this, children.mapIndexed { index, child ->
                    val params = when {
                        horizontal && kindOf(child) == "text" -> LinearLayout.LayoutParams(0, -2, 1f)
                        // An end-aligned glyph button takes the rest of its
                        // row and sits at its end, as a toolbar button does.
                        horizontal && endGlyph(child) -> LinearLayout.LayoutParams(0, -2, 1f)
                        horizontal -> LinearLayout.LayoutParams(-2, -2)
                        !scrolling && fills(child) -> LinearLayout.LayoutParams(-1, 0, 1f)
                        else -> LinearLayout.LayoutParams(-1, -2)
                    }
                    if (index > 0) { if (horizontal) params.marginStart = gap else params.topMargin = gap }
                    if (horizontal) params.gravity = Gravity.CENTER_VERTICAL
                    node(child, depth + 1) to params
                })
            }
            "stack:float" -> (view as LinearLayout).let { outer ->
                val gap = space(style.textOrNull("gap"))
                val children = props.getJSONArray("children").objects()
                val split = floatSplit(children)!!
                val area = mounted.area!!
                val group = mounted.group!!
                val before = children.take(split).mapIndexed { index, child ->
                    node(child, depth + 1) to LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = gap }
                }
                replaceChildren(outer, before + (area to LinearLayout.LayoutParams(-1, 0, 1f).apply {
                    if (split > 0) topMargin = gap }))
                val transcript = node(children[split], depth + 1)
                if (area.getChildAt(0) !== transcript) {
                    (transcript.parent as? ViewGroup)?.removeView(transcript)
                    area.removeAllViews()
                    area.addView(transcript, FrameLayout.LayoutParams(-1, -1))
                    area.addView(group, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
                }
                replaceChildren(group, children.drop(split + 1).mapIndexed { index, child ->
                    node(child, depth + 1) to LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = gap }
                })
                mounted.floated = mounts[children[split].getString("key")]?.transcript
                mounted.floated?.setBottomInset(group.height)
            }
            "stack:wrap" -> (view as NativeFlow).apply {
                gap = space(style.textOrNull("gap"))
                val children = props.getJSONArray("children").objects().map { node(it, depth + 1) }
                children.forEachIndexed { index, child ->
                    if (getChildAt(index) !== child) {
                        (child.parent as? ViewGroup)?.removeView(child)
                        addView(child, index.coerceAtMost(childCount), ViewGroup.LayoutParams(-2, -2))
                    }
                }
                while (childCount > children.size) removeViewAt(childCount - 1)
                requestLayout()
            }
            "stack:menu" -> (view as FrameLayout).let { frame ->
                // The card, whose long press offers the other buttons as a
                // native menu (PopupMenu). Each item activates its own node,
                // so a menu from an older view is refused.
                val children = props.getJSONArray("children").objects()
                val card = node(children.first(), depth + 1)
                val items = children.drop(1).map { item ->
                    val itemProps = item.getJSONObject("element").getJSONObject("props")
                    Triple(item.getString("key"), itemProps.optString("label"), itemProps.optBoolean("enabled"))
                }
                if (frame.childCount != 1 || frame.getChildAt(0) !== card) {
                    (card.parent as? ViewGroup)?.removeView(card)
                    frame.removeAllViews()
                    frame.addView(card, FrameLayout.LayoutParams(-1, -2))
                }
                card.setOnLongClickListener { anchor ->
                    val menu = android.widget.PopupMenu(context, anchor)
                    items.forEachIndexed { index, (_, label, enabled) ->
                        menu.menu.add(0, index, index, label).isEnabled = enabled
                    }
                    menu.setOnMenuItemClickListener { item ->
                        items.getOrNull(item.itemId)?.let { (itemKey, _, _) -> activateNode(itemKey) }
                        true
                    }
                    menu.show()
                    true
                }
                card.setTag(R.id.native_menu, items.map { it.first })
            }
            "list" -> {
                view.contentDescription = props.getString("label")
                replaceChildren(mounted.rows!!, props.getJSONArray("children").objects().map { child ->
                    node(child, depth + 1) to LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = context.dp(8) }
                })
            }
            "button" -> (view as Button).apply {
                val label = props.getString("label")
                if (text.toString() != label) text = label
                isEnabled = props.getBoolean("enabled")
                alpha = if (isEnabled) 1f else 0.4f
            }
            "button:circle", "button:link", "button:pill" -> {
                val label = props.getString("label")
                val enabled = props.getBoolean("enabled")
                view.isEnabled = enabled; view.alpha = if (enabled) 1f else 0.4f
                view.contentDescription = label
                val inner = (view as FrameLayout).getChildAt(0)
                (inner.layoutParams as FrameLayout.LayoutParams).gravity =
                    (if (style.textOrNull("align") == "end") Gravity.END else Gravity.START) or Gravity.CENTER_VERTICAL
                if (kind != "button:circle") (inner as TextView).apply {
                    if (text.toString() != label) text = label
                    val size = context.dp(if (kind == "button:pill") 16 else 18)
                    // The glyphs are drawn white; they take the theme's text color.
                    val icon = context.getDrawable(glyph(props)!!)?.mutate()?.apply {
                        setBounds(0, 0, size, size); setTint(Palette.PRIMARY)
                    }
                    setCompoundDrawables(icon, null, null, null)
                } else (inner as android.widget.ImageView).apply {
                    setImageResource(glyph(props)!!); setColorFilter(Palette.PRIMARY)
                }
                inner.requestLayout()
            }
            "surface" -> (view as FrameLayout).let { frame ->
                val value = props.getString("resource")
                // An attached image speaks its alternative text.
                if (value.startsWith("image:")) frame.contentDescription = props.getString("label")
                val drawn = surfaces?.invoke(value)
                    ?: context.text("This device can't display ${props.getString("label")}.", 14f, Palette.SECONDARY)
                // The same view again (its content unchanged) stays mounted,
                // so a card keeps its scroll and a drag in progress.
                if (frame.childCount != 1 || frame.getChildAt(0) !== drawn) {
                    (drawn.parent as? android.view.ViewGroup)?.removeView(drawn)
                    frame.removeAllViews()
                    // A stretched surface fills its frame, as the new chat's cards.
                    frame.addView(drawn, FrameLayout.LayoutParams(-1, if (style.optBoolean("fill_height")) -1 else -2))
                }
            }
            "transcript" -> mounted.transcript!!.update(props)
            "composer" -> mounted.composer!!.update(props)
            else -> if (kind.startsWith("text:")) {
                val value = props.getString("value")
                if (mounted.value != value) { mounted.value = value; mounted.text!!.text = value }
            } else {
                // Messages, Markdown, tool rows, and working rows outside a
                // transcript: rebuild only when their content changes.
                val value = node.toString()
                if (mounted.value != value) {
                    mounted.value = value
                    (view as FrameLayout).removeAllViews()
                    view.addView(chat.build(node, depth), FrameLayout.LayoutParams(-1, -2))
                }
            }
        }
        view.setTag(R.id.native_key, key)
        if (!kind.startsWith("button") && kind != "composer") view.setPadding(space(style.textOrNull("padding_start")), space(style.textOrNull("padding_top")),
            space(style.textOrNull("padding_end")), space(style.textOrNull("padding_bottom")))
        style.objectOrNull("background")?.let { view.setBackgroundColor(color(it)) }
            ?: if (!kind.startsWith("button") && kind != "transcript") view.setBackgroundColor(Color.TRANSPARENT) else Unit
        (mounted.text ?: view as? TextView)?.let { text ->
            text.setTextColor(style.objectOrNull("foreground")?.let { color(it) }
                ?: if (kind == "text:status") Palette.SECONDARY else Palette.PRIMARY)
            val bold = style.textOrNull("weight") == "bold" || kind == "text:heading"
            val weight = if (bold) Fonts.BOLD else Fonts.REGULAR
            val mono = kind == "text:code" || kind == "text:terminal" || style.optBoolean("monospace")
            text.typeface = if (mono) Fonts.code(text.context, weight) else Fonts.typeface(text.context, weight)
            text.gravity = if (kind == "button") Gravity.CENTER else when (style.textOrNull("align")) {
                "center" -> Gravity.CENTER_HORIZONTAL; "end" -> Gravity.END; else -> Gravity.START
            }
        }
        return view
    }

    private fun create(kind: String, props: JSONObject): Mounted = when {
        kind.startsWith("text:") -> {
            val role = kind.substringAfter(':')
            val text = context.text("", when (role) {
                "heading" -> 20f; "status" -> 13f; "code" -> 14f; "terminal" -> TerminalMetrics.SIZE_SP; else -> 16f
            })
            if (role == "terminal") {
                text.maxLines = 1; text.setHorizontallyScrolling(true); text.includeFontPadding = false
                text.setLineSpacing(0f, 1f)
            } else text.setTextIsSelectable(true)
            if (role == "heading" && android.os.Build.VERSION.SDK_INT >= 28) text.isAccessibilityHeading = true
            Mounted(kind, text).also { it.text = text }
        }
        kind == "button" -> Mounted(kind, Button(context).apply {
            isAllCaps = false; setTextColor(Palette.PRIMARY); textSize = 15f
            background = context.rounded(Palette.RAISED, 10f)
            minHeight = context.dp(44); minimumHeight = context.dp(44)
            setPadding(context.dp(14), 0, context.dp(14), 0)
            stateListAnimator = null
            setOnClickListener { v -> (v.getTag(R.id.native_key) as? String)?.let { activateNode(it) } }
        })
        kind == "button:circle" -> Mounted(kind, FrameLayout(context).apply {
            // A 44 dp circle with the glyph; the label is its spoken name.
            addView(android.widget.ImageView(context).apply {
                scaleType = android.widget.ImageView.ScaleType.CENTER
                background = context.rounded(Palette.RAISED, 22f, Palette.BORDER)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, FrameLayout.LayoutParams(context.dp(44), context.dp(44)))
            isClickable = true; isFocusable = true
            setOnClickListener { v -> if (v.isEnabled) (v.getTag(R.id.native_key) as? String)?.let { activateNode(it) } }
        })
        kind == "button:link" -> Mounted(kind, FrameLayout(context).apply {
            // The glyph before a visible label, as a back link.
            addView(context.text("", 17f).apply {
                compoundDrawablePadding = context.dp(4)
                setPadding(0, context.dp(10), context.dp(8), context.dp(10))
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, FrameLayout.LayoutParams(-2, -2))
            isClickable = true; isFocusable = true
            setOnClickListener { v -> if (v.isEnabled) (v.getTag(R.id.native_key) as? String)?.let { activateNode(it) } }
        })
        kind == "button:pill" -> Mounted(kind, FrameLayout(context).apply {
            // A chip: the glyph and one line of label in a capsule.
            addView(context.text("", 15f).apply {
                compoundDrawablePadding = context.dp(6)
                gravity = Gravity.CENTER_VERTICAL
                maxLines = 1; ellipsize = android.text.TextUtils.TruncateAt.MIDDLE
                minHeight = context.dp(36)
                setPadding(context.dp(14), 0, context.dp(14), 0)
                background = context.rounded(Palette.RAISED, 18f, Palette.BORDER)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, FrameLayout.LayoutParams(-2, -2))
            isClickable = true; isFocusable = true
            setOnClickListener { v -> if (v.isEnabled) (v.getTag(R.id.native_key) as? String)?.let { activateNode(it) } }
        })
        kind == "stack:wrap" -> Mounted(kind, NativeFlow(context))
        kind == "stack:menu" -> Mounted(kind, FrameLayout(context))
        kind == "stack" -> Mounted(kind, context.column())
        kind == "stack:float" -> {
            val group = context.column().apply {
                // A soft fade from the conversation into the theme's background.
                background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.TOP_BOTTOM,
                    intArrayOf(clearOf(Palette.BACKGROUND), Palette.BACKGROUND, Palette.BACKGROUND))
                setPadding(0, context.dp(20), 0, 0)
            }
            val mounted = Mounted(kind, context.column())
            group.addOnLayoutChangeListener { _, _, top, _, bottom, _, oldTop, _, oldBottom ->
                if (bottom - top != oldBottom - oldTop) mounted.floated?.setBottomInset(bottom - top)
            }
            mounted.also { it.area = FrameLayout(context); it.group = group }
        }
        kind == "list" -> {
            val rows = context.column()
            val outer: View = if (scrolling) rows else ScrollView(context).apply {
                isFillViewport = true; addView(rows)
            }
            Mounted(kind, outer).also { it.rows = rows }
        }
        kind == "surface" -> Mounted(kind, FrameLayout(context))
        kind == "transcript" -> {
            val transcript = RustTranscript(context, surfaces) { key -> activateNode(key) }
            Mounted(kind, transcript.root).also { it.transcript = transcript }
        }
        kind == "composer" -> {
            val composer = Composer(context, { token, value -> submit?.invoke(token, value) }, { key -> activateNode(key) })
            Mounted(kind, composer.root).also { it.composer = composer }
        }
        kind in setOf("message", "markdown", "tool", "working") -> Mounted(kind, FrameLayout(context))
        else -> throw IllegalArgumentException("This app does not support a returned native component.")
    }

    private fun kindOf(node: JSONObject) = node.getJSONObject("element").getString("kind")

    /** The drawable for a button's glyph, or null when it has none this app knows. */
    private fun glyph(props: JSONObject): Int? = when (props.objectOrNull("icon")?.optString("glyph")) {
        "back" -> R.drawable.ic_glyph_back
        "compose" -> R.drawable.ic_glyph_compose
        "menu" -> R.drawable.ic_glyph_menu
        "history" -> R.drawable.ic_glyph_history
        "folder" -> R.drawable.ic_glyph_folder
        "computer" -> R.drawable.ic_glyph_computer
        "cloud" -> R.drawable.ic_glyph_cloud
        "add" -> R.drawable.ic_glyph_add
        "check" -> R.drawable.ic_glyph_check
        "stop" -> R.drawable.ic_glyph_stop
        "ask" -> R.drawable.ic_glyph_ask
        "flag" -> R.drawable.ic_glyph_flag
        "terminal" -> R.drawable.ic_glyph_terminal
        "wallet" -> R.drawable.ic_glyph_wallet
        "key" -> R.drawable.ic_glyph_key
        "person" -> R.drawable.ic_glyph_person
        "paperclip" -> R.drawable.ic_glyph_paperclip
        else -> null
    }

    private fun endGlyph(node: JSONObject): Boolean {
        val element = node.getJSONObject("element")
        return element.getString("kind") == "button" && glyph(element.getJSONObject("props")) != null &&
            node.optJSONObject("style")?.textOrNull("align") == "end"
    }

    /**
     * In a floating root stack, the transcript's index when something
     * follows it (the composer); null when the stack doesn't float.
     */
    private fun floatSplit(children: List<JSONObject>): Int? {
        val index = children.indexOfFirst { kindOf(it) == "transcript" }
        return index.takeIf { it >= 0 && it < children.size - 1 }
    }

    /** A node that takes the remaining height in a vertical stack. */
    private fun fills(node: JSONObject): Boolean {
        val element = node.getJSONObject("element")
        return when (element.getString("kind")) {
            "list", "transcript" -> true
            // A surface the application stretches, as the new chat's cards.
            "surface" -> node.optJSONObject("style")?.optBoolean("fill_height") == true
            "stack" -> element.getJSONObject("props").let { props ->
                props.getString("axis") == "vertical" && props.getJSONArray("children").objects().any { fills(it) }
            }
            else -> false
        }
    }

    private fun replaceChildren(parent: LinearLayout, children: List<Pair<View, LinearLayout.LayoutParams>>) {
        children.forEachIndexed { index, (view, params) ->
            if (parent.getChildAt(index) !== view) {
                (view.parent as? ViewGroup)?.removeView(view)
                parent.addView(view, index.coerceAtMost(parent.childCount), params)
            } else view.layoutParams = params
        }
        while (parent.childCount > children.size) parent.removeViewAt(parent.childCount - 1)
    }

    private fun space(value: String?) = context.dp(when (value) {
        "xs" -> 4; "sm" -> 8; "md" -> 16; "lg" -> 24; else -> 0
    })

    companion object {
        fun color(value: JSONObject) =
            Color.argb(value.getInt("alpha"), value.getInt("red"), value.getInt("green"), value.getInt("blue"))
    }
}

/** The cell font for Rust Native's terminal role; the terminal screen sizes its grid from it. */
object TerminalMetrics {
    const val SIZE_SP = 12f
    fun cell(context: Context): Pair<Float, Float> {
        val paint = android.text.TextPaint().apply {
            typeface = Fonts.code(context)
            textSize = android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
                SIZE_SP, context.resources.displayMetrics)
        }
        val metrics = paint.fontMetrics
        return paint.measureText("M") to (metrics.descent - metrics.ascent)
    }
}

/** Children left to right at their own sizes, continuing on the next line when the next does not fit. */
class NativeFlow(context: Context) : ViewGroup(context) {
    var gap = 0

    override fun onMeasure(widthSpec: Int, heightSpec: Int) {
        val width = (MeasureSpec.getSize(widthSpec) - paddingLeft - paddingRight).coerceAtLeast(0)
        for (index in 0 until childCount) getChildAt(index).measure(
            MeasureSpec.makeMeasureSpec(width, MeasureSpec.AT_MOST),
            MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED))
        val height = place(width) { _, _, _ -> }
        setMeasuredDimension(MeasureSpec.getSize(widthSpec), height + paddingTop + paddingBottom)
    }

    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        place(r - l - paddingLeft - paddingRight) { child, x, y ->
            child.layout(paddingLeft + x, paddingTop + y, paddingLeft + x + child.measuredWidth, paddingTop + y + child.measuredHeight)
        }
    }

    /** Visits each measured child at its place; returns the height the lines use. */
    private fun place(width: Int, visit: (View, Int, Int) -> Unit): Int {
        var x = 0; var y = 0; var line = 0
        for (index in 0 until childCount) {
            val child = getChildAt(index)
            if (x > 0 && x + child.measuredWidth > width) { x = 0; y += line + gap; line = 0 }
            visit(child, x, y)
            x += child.measuredWidth + gap
            line = maxOf(line, child.measuredHeight)
        }
        return y + line
    }
}
