package com.openagents.app

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
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
 * Supported: stack, list, text (every role), button, surface (as a refusal),
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
) {
    private class Mounted(val kind: String, val view: View) {
        var value: String? = null
        var text: TextView? = null
        var rows: LinearLayout? = null
        var transcript: Transcript? = null
        var composer: Composer? = null
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
        mounts.keys.retainAll(seen)
    }

    fun clear() { currentView = null; currentInstance = null; currentRevision = -1; mounts.clear() }

    private fun activateNode(key: String) { currentView?.let { activate(it, key) } }

    private fun node(node: JSONObject, depth: Int): View {
        require(depth <= 16 && seen.size < 1024) { "The native view exceeds its size limit." }
        val key = node.getString("key")
        require(seen.add(key)) { "The native view repeats a key." }
        val element = node.getJSONObject("element")
        val props = element.getJSONObject("props")
        val style = node.optJSONObject("style") ?: JSONObject()
        val elementKind = element.getString("kind")
        val kind = if (elementKind == "text") "text:${props.getString("role")}" else elementKind
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
                        horizontal -> LinearLayout.LayoutParams(-2, -2)
                        !scrolling && fills(child) -> LinearLayout.LayoutParams(-1, 0, 1f)
                        else -> LinearLayout.LayoutParams(-1, -2)
                    }
                    if (index > 0) { if (horizontal) params.marginStart = gap else params.topMargin = gap }
                    if (horizontal) params.gravity = Gravity.CENTER_VERTICAL
                    node(child, depth + 1) to params
                })
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
            "surface" -> (view as TextView).text = "This device can't display ${props.getString("label")}."
            "transcript" -> mounted.transcript!!.update(props, chat)
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
        if (kind != "button" && kind != "composer") view.setPadding(space(style.textOrNull("padding_start")), space(style.textOrNull("padding_top")),
            space(style.textOrNull("padding_end")), space(style.textOrNull("padding_bottom")))
        style.objectOrNull("background")?.let { view.setBackgroundColor(color(it)) }
            ?: if (kind != "button" && kind != "transcript") view.setBackgroundColor(Color.TRANSPARENT) else Unit
        (mounted.text ?: view as? TextView)?.let { text ->
            text.setTextColor(style.objectOrNull("foreground")?.let { color(it) }
                ?: if (kind == "text:status") Palette.SECONDARY else Palette.PRIMARY)
            val bold = style.textOrNull("weight") == "bold" || kind == "text:heading"
            text.setTypeface(if (kind == "text:code" || kind == "text:terminal") Typeface.MONOSPACE else Typeface.DEFAULT,
                if (bold) Typeface.BOLD else Typeface.NORMAL)
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
        kind == "stack" -> Mounted(kind, context.column())
        kind == "list" -> {
            val rows = context.column()
            val outer: View = if (scrolling) rows else ScrollView(context).apply {
                isFillViewport = true; addView(rows)
            }
            Mounted(kind, outer).also { it.rows = rows }
        }
        kind == "surface" -> Mounted(kind, context.text("", 14f, Palette.SECONDARY))
        kind == "transcript" -> {
            val transcript = Transcript(context) { key -> activateNode(key) }
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

    /** A node that takes the remaining height in a vertical stack. */
    private fun fills(node: JSONObject): Boolean {
        val element = node.getJSONObject("element")
        return when (element.getString("kind")) {
            "list", "transcript" -> true
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
            typeface = Typeface.MONOSPACE
            textSize = android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
                SIZE_SP, context.resources.displayMetrics)
        }
        val metrics = paint.fontMetrics
        return paint.measureText("M") to (metrics.descent - metrics.ascent)
    }
}
