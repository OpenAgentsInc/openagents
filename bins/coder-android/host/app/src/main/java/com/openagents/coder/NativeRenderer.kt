package com.openagents.coder

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.text.SpannableString
import android.text.style.ClickableSpan
import android.text.style.ImageSpan
import android.view.Gravity
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import io.noties.markwon.Markwon
import org.json.JSONObject

internal const val AMBER = 0xffffb000.toInt()
internal const val BACKGROUND = 0xff060500.toInt()
internal fun Context.dp(value: Int) = (value * resources.displayMetrics.density).toInt()
internal fun View.identifier(value: String): View { tag = value; return this }
internal fun Context.label(value: String, key: String? = null, size: Float = 14f): TextView = TextView(this).apply {
    text = value; textSize = size; setTextColor(AMBER); setTextIsSelectable(true); isSaveEnabled = false
    key?.let { tag = it }
}
internal fun Context.button(value: String, key: String, action: () -> Unit): Button = Button(this).apply {
    text = value; tag = key; isAllCaps = false; setTextColor(AMBER)
    setOnClickListener { action() }
}
internal fun Context.column() = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

/** Native controls reconcile by stable key; text selection survives unchanged content.
 *  `viewKey` names the packet field that holds this renderer's view. */
class NativeRenderer(private val context: Context, private val activate: (JSONObject, String) -> Unit,
                     private val follow: (Boolean, String) -> Unit, private val viewKey: String = "view") {
    private data class Mounted(val kind: String, val view: View, var value: String? = null,
        var original: Boolean = false, var following: Boolean = false,
        var followPage: String? = null, var followTarget: String? = null,
        var text: TextView? = null,
        var scroll: ScrollView? = null, var children: LinearLayout? = null, var toggle: Switch? = null)
    private val mounts = mutableMapOf<String, Mounted>()
    private val markwon = Markwon.create(context)
    private var currentView: JSONObject? = null
    private var currentInstance: String? = null
    private var currentRevision = -1L
    private var seen = mutableSetOf<String>()

    fun mount(container: LinearLayout, packet: JSONObject) {
        val view = packet.optJSONObject(viewKey)
        if (view == null) { container.removeAllViews(); clear(); return }
        require(view.getString("schema") == "rust-native.view.v2") { "Unsupported native view." }
        val instance = view.getString("instance")
        val revision = view.getLong("revision")
        if (instance == currentInstance && revision == currentRevision && container.childCount > 0) return
        if (instance != currentInstance) { container.removeAllViews(); clear() }
        seen = mutableSetOf()
        val root = node(view.getJSONObject("root"), packet, 0)
        replaceChildren(container, listOf(root to LinearLayout.LayoutParams(-1, -1)))
        mounts.keys.retainAll(seen)
        currentView = view; currentInstance = instance; currentRevision = revision
    }

    fun clear() { currentView = null; currentInstance = null; currentRevision = -1; mounts.clear() }

    private fun node(node: JSONObject, packet: JSONObject, depth: Int): View {
        require(depth <= 16 && seen.size < 1024) { "The native view exceeds its size limit." }
        val key = node.getString("key")
        require(seen.add(key)) { "The native view repeats a key." }
        val element = node.getJSONObject("element")
        val props = element.getJSONObject("props")
        val style = node.optJSONObject("style") ?: JSONObject()
        val kind = element.getString("kind") + if (element.getString("kind") == "text") ":${props.getString("role")}" else ""
        val existing = mounts[key]?.takeIf { it.kind == kind }
        val mounted = existing ?: create(key, kind, props).also { mounts[key] = it }
        val result = mounted.view
        when {
            kind.startsWith("text:") -> {
                val value = props.getString("value")
                if (mounted.value != value) { mounted.value = value; updateText(mounted, kind.substringAfter(':')) }
            }
            kind == "button" -> (result as Button).apply {
                if (text.toString() != props.getString("label")) text = props.getString("label")
                isEnabled = props.getBoolean("enabled")
            }
            kind == "stack" -> (result as LinearLayout).apply {
                orientation = if (props.getString("axis") == "horizontal") LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
                val children = props.getJSONArray("children")
                val gap = space(style.optString("gap"))
                val rows = (0 until children.length()).map { index ->
                    val child = children.getJSONObject(index)
                    val childKind = child.getJSONObject("element").getString("kind")
                    val params = if (orientation == LinearLayout.HORIZONTAL) LinearLayout.LayoutParams(0, -2, 1f)
                        else if (childKind == "list") LinearLayout.LayoutParams(-1, 0, 1f)
                        else LinearLayout.LayoutParams(-1, -2)
                    if (index > 0) { if (orientation == LinearLayout.HORIZONTAL) params.marginStart = gap else params.topMargin = gap }
                    node(child, packet, depth + 1) to params
                }
                replaceChildren(this, rows)
            }
            kind == "list" -> {
                val children = props.getJSONArray("children")
                mounted.followPage = packet.textOrNull("follow_page")
                mounted.followTarget = packet.textOrNull("follow_target")
                mounted.following = mounted.followTarget != null
                mounted.toggle!!.apply {
                    visibility = if (mounted.followPage == null) View.GONE else View.VISIBLE
                    if (isChecked != mounted.following) isChecked = mounted.following
                }
                mounted.scroll!!.contentDescription = props.getString("label")
                val rows = (0 until children.length()).map { index -> node(children.getJSONObject(index), packet, depth + 1) to
                    LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = context.dp(8) } }
                replaceChildren(mounted.children!!, rows)
                val target = mounted.followTarget
                if (mounted.following && target != null) mounted.scroll!!.post {
                    if (mounts[key] === mounted && mounted.following) {
                        val row = mounted.children!!.findViewWithTag<View>(target)
                        if (row != null) mounted.scroll!!.smoothScrollTo(0, (row.bottom - mounted.scroll!!.height).coerceAtLeast(0))
                    }
                }
            }
        }
        result.tag = key
        result.setPadding(space(style.optString("padding_start")), space(style.optString("padding_top")),
            space(style.optString("padding_end")), space(style.optString("padding_bottom")))
        result.setBackgroundColor(style.optJSONObject("background")?.let { color(it) } ?: Color.TRANSPARENT)
        val text = mounted.text ?: result as? TextView
        text?.let {
            it.setTextColor(style.optJSONObject("foreground")?.let { c -> color(c) } ?: AMBER)
            if (style.optString("weight") == "bold") it.setTypeface(it.typeface, Typeface.BOLD)
            it.gravity = when (style.optString("align")) { "center" -> Gravity.CENTER; "end" -> Gravity.END; else -> Gravity.START }
        }
        return result
    }

    private fun create(key: String, kind: String, props: JSONObject): Mounted {
        if (kind.startsWith("text:")) {
            val role = kind.substringAfter(':')
            val text = context.label("", if (role == "markdown") "$key-text" else key)
            if (role == "heading") { text.textSize = 18f; text.setTypeface(text.typeface, Typeface.BOLD); if (android.os.Build.VERSION.SDK_INT >= 28) text.isAccessibilityHeading = true }
            if (role == "code") text.typeface = Typeface.MONOSPACE
            // A terminal grid row: monospaced, one line, never wrapped.
            if (role == "terminal") { text.typeface = Typeface.MONOSPACE; text.textSize = 12f; text.maxLines = 1; text.setHorizontallyScrolling(true) }
            if (role == "status") text.textSize = 12f
            text.autoLinkMask = 0
            if (role != "markdown") return Mounted(kind, text, text = text)
            val outer = context.column(); outer.addView(text)
            val mounted = Mounted(kind, outer, text = text)
            val sourceAction = View.generateViewId()
            text.customSelectionActionModeCallback = object : ActionMode.Callback {
                override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
                    menu.add(Menu.NONE, sourceAction, Menu.NONE, if (mounted.original) "Formatted text" else "Original Markdown")
                    return true
                }
                override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false
                override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
                    if (item.itemId != sourceAction) return false
                    mounted.original = !mounted.original; updateText(mounted, role); mode.finish()
                    return true
                }
                override fun onDestroyActionMode(mode: ActionMode) = Unit
            }
            return mounted
        }
        return when (kind) {
            "button" -> Mounted(kind, context.button(props.getString("label"), key) {
                currentView?.let { activate(it, key) }
            })
            "stack" -> Mounted(kind, context.column())
            "list" -> {
                val outer = context.column()
                val scroll = ScrollView(context).apply { isFillViewport = true }
                val rows = context.column()
                val toggle = Switch(context).apply { text = "Follow new messages"; setTextColor(AMBER); tag = "$key-follow" }
                val mounted = Mounted(kind, outer, scroll = scroll, children = rows, toggle = toggle)
                toggle.setOnCheckedChangeListener { _, checked ->
                    if (mounted.following != checked) { mounted.following = checked; mounted.followPage?.let { follow(checked, it) } }
                }
                scroll.setOnTouchListener { _, event ->
                    if (mounted.followPage != null && mounted.following && event.actionMasked == MotionEvent.ACTION_MOVE) toggle.isChecked = false
                    false
                }
                outer.addView(toggle); scroll.addView(rows); outer.addView(scroll, LinearLayout.LayoutParams(-1, 0, 1f))
                mounted
            }
            "surface" -> Mounted(kind, context.label("This reader cannot display ${props.getString("label")}.", "$key-unsupported"))
            else -> throw IllegalArgumentException("This app does not support a returned native component.")
        }
    }
    private fun updateText(mounted: Mounted, role: String) {
        val value = mounted.value.orEmpty()
        mounted.text!!.text = if (role == "markdown" && !mounted.original) {
            SpannableString(markwon.toMarkdown(value)).apply {
                getSpans(0, length, ClickableSpan::class.java).forEach { removeSpan(it) }
                getSpans(0, length, ImageSpan::class.java).forEach { removeSpan(it) }
            }
        } else value
        mounted.text!!.linksClickable = false
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
    private fun color(value: JSONObject) = Color.argb(value.getInt("alpha"), value.getInt("red"), value.getInt("green"), value.getInt("blue"))
    private fun space(value: String) = context.dp(when (value) { "xs" -> 4; "sm" -> 8; "md" -> 16; "lg" -> 24; else -> 0 })
}
