package com.openagents.app

import android.app.Dialog
import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Looper
import android.text.Editable
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.TextWatcher
import android.text.style.*
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.BaseInputConnection
import android.widget.*
import com.openagents.app.accessibility.NativeSemantics
import org.json.JSONObject

/** Mounts only JSON prepared by the Rust AndroidView bridge. No application intents cross back. */
class SemanticViewAdapter(
    private val context: Context,
    private val activate: (instance: String, revision: Long, key: String) -> Unit,
    private val changeField: (instance: String, revision: Long, key: String, value: String) -> Unit,
    private val acknowledge: (instance: String, revision: Long) -> Unit,
    // Transcript, composer, Markdown, and local surfaces use their dedicated adapters.
    private val extensions: Map<String, Extension> = emptyMap(),
    private val semantics: NativeSemantics? = null,
) {
    interface Extension {
        fun mount(node: JSONObject, previous: View?): View
        fun dispose(view: View)
    }
    private data class Control(val kind: String, val view: View, var watcher: TextWatcher? = null, var secret: Boolean = false, val checkbox: Boolean = false)
    private val controls = mutableMapOf<String, Control>()
    private val dialogs = mutableMapOf<String, Dialog>()
    private val popups = mutableSetOf<PopupMenu>()
    private val origins = mutableMapOf<String, View?>()
    private var current: JSONObject? = null
    private var disposed = false
    private var updating = false
    private var container: ViewGroup? = null
    private val builtins = setOf("stack", "list", "text", "rich_text", "button", "field", "choice", "dialog", "working", "message", "tool")

    /** Call on the UI thread. Acknowledgment follows a successful mount, never receipt. */
    fun mount(parent: ViewGroup, json: ByteArray) {
        check(Looper.myLooper() == Looper.getMainLooper())
        check(!disposed)
        require(json.size <= 512 * 1024)
        val view = JSONObject(String(json, Charsets.UTF_8))
        require(view.getString("schema") in setOf("rust-native.view.v2", "rust-native.view.v3"))
        val instance = view.getString("instance")
        val revision = view.getLong("revision")
        require(revision > 0)
        current?.let {
            require(it.getString("instance") == instance && revision > it.getLong("revision"))
        }
        require(container == null || container === parent)
        val keys = mutableSetOf<String>()
        fun checkNode(node: JSONObject, depth: Int) {
            require(depth <= 16 && keys.size < 1024 && keys.add(node.getString("key")))
            val element = node.getJSONObject("element")
            val kind = element.getString("kind")
            require(kind in builtins || extensions.containsKey(kind)) { "Unsupported view: $kind" }
            val children = element.getJSONObject("props").optJSONArray("children")
            if (children != null) for (i in 0 until children.length()) checkNode(children.getJSONObject(i), depth + 1)
        }
        require(json.size <= 512 * 1024)
        checkNode(view.getJSONObject("root"), 1)
        val semanticTree = semantics?.prepare(json)
        updating = true
        try {
            val seen = mutableSetOf<String>()
            val root = node(view.getJSONObject("root"), seen)
            children(parent, listOf(root))
            (controls.keys - seen).forEach { release(it) }
            container = parent
            current = view
        } catch (error: Exception) {
            dispose()
            throw error
        } finally {
            updating = false
        }
        try {
            acknowledge(instance, revision)
            if (semanticTree != null) semantics?.mounted(semanticTree, controls.mapValues { (_, control) ->
                if (control.kind == "choice") (control.view as ViewGroup).getChildAt(0) else control.view
            }) { key -> event(key) }
        } catch (error: Exception) {
            dispose()
            throw error
        }
    }

    private fun event(key: String, value: String? = null) {
        if (updating || disposed) return
        val view = current ?: return
        if (value == null) activate(view.getString("instance"), view.getLong("revision"), key)
        else changeField(view.getString("instance"), view.getLong("revision"), key, value)
    }

    private fun column() = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private fun node(node: JSONObject, seen: MutableSet<String>): View {
        val key = node.getString("key")
        seen.add(key)
        val element = node.getJSONObject("element")
        val kind = element.getString("kind")
        val props = element.getJSONObject("props")
        val checkbox = kind == "button" && props.optJSONObject("icon")?.optString("glyph") in setOf("checked", "unchecked")
        val old = controls[key]
        if (old != null && (old.kind != kind || old.checkbox != checkbox)) release(key)
        val control = controls[key] ?: Control(kind, when (kind) {
            "button" -> if (checkbox) CheckBox(context) else Button(context)
            "choice" -> column()
            "field" -> EditText(context)
            "text", "rich_text", "working" -> TextView(context)
            "list" -> ScrollView(context).apply { addView(column()) }
            "stack", "dialog", "message", "tool" -> column()
            else -> extensions.getValue(kind).mount(node, null)
        }, checkbox = checkbox).also { controls[key] = it }
        val result = control.view
        result.tag = key
        result.isSaveEnabled = false
        val nested = props.optJSONArray("children")
        fun childViews(): List<View> = if (nested == null) emptyList() else
            (0 until nested.length()).map { node(nested.getJSONObject(it), seen) }
        when (kind) {
            "button" -> {
                val button = result as Button
                button.text = props.getString("label")
                button.contentDescription = props.getString("label")
                button.isEnabled = props.getBoolean("enabled")
                button.isAllCaps = false
                button.minHeight = dp(48)
                val selected = props.optJSONObject("icon")?.optString("glyph") == "checked"
                if (button is CheckBox) button.isChecked = selected
                button.setOnClickListener {
                    if (button is CheckBox) button.isChecked = selected
                    event(key)
                }
            }
            "choice" -> {
                val layout = result as LinearLayout
                val check = (layout.getChildAt(0) as? RadioButton) ?: RadioButton(context)
                check.text = props.getString("label")
                check.isEnabled = props.getBoolean("enabled")
                check.isChecked = props.getBoolean("selected")
                check.minHeight = dp(48)
                check.setOnClickListener { check.isChecked = props.getBoolean("selected"); event(key) }
                children(layout, listOf(check) + childViews())
            }
            "field" -> {
                val field = result as EditText
                control.watcher?.let { field.removeTextChangedListener(it) }
                field.contentDescription = props.getString("label")
                field.hint = props.getString("placeholder")
                field.isEnabled = props.getBoolean("enabled")
                val secret = props.getBoolean("secret")
                val multiline = props.getBoolean("multiline")
                if (secret != control.secret) field.text.clear()
                control.secret = secret
                field.minHeight = dp(48)
                field.filters = arrayOf(android.text.InputFilter { source, start, end, dest, dstart, dend ->
                    val replacement = source.subSequence(start, end).toString()
                    val proposed = dest.subSequence(0, dstart).toString() + replacement + dest.subSequence(dend, dest.length)
                    if (proposed.toByteArray(Charsets.UTF_8).size > props.getInt("max_bytes") ||
                        (!multiline && (proposed.contains('\n') || proposed.contains('\r')))) "" else null
                })
                val input = android.text.InputType.TYPE_CLASS_TEXT or when {
                    secret -> android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD
                    multiline -> android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE
                    else -> android.text.InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
                }
                if (field.inputType != input) field.inputType = input
                field.setSingleLine(!multiline)
                val value = props.getString("value")
                // Keep private input and active IME composition across Rust refreshes.
                if (!secret && BaseInputConnection.getComposingSpanStart(field.text) < 0 && field.text.toString() != value) {
                    val selection = field.selectionStart.coerceAtLeast(0).coerceAtMost(value.length)
                    field.setText(value)
                    field.setSelection(selection)
                }
                val watcher = object : TextWatcher {
                    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
                    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
                    override fun afterTextChanged(s: Editable?) {
                        val text = s.toString()
                        if (text.toByteArray(Charsets.UTF_8).size <= props.getInt("max_bytes") &&
                            (multiline || (!text.contains('\n') && !text.contains('\r')))) event(key, text)
                    }
                }
                field.addTextChangedListener(watcher)
                control.watcher = watcher
            }
            "text", "working" -> {
                val text = result as TextView
                val value = props.getString(if (kind == "working") "label" else "value")
                if (text.text.toString() != value) text.text = value
                text.setTextIsSelectable(kind == "text")
                if (Build.VERSION.SDK_INT >= 28) text.isAccessibilityHeading = props.optString("role") == "heading"
                else text.contentDescription = if (props.optString("role") == "heading") "Heading. $value" else null
                val terminal = props.optString("role") == "terminal"
                text.setSingleLine(terminal)
                text.typeface = if (terminal || props.optString("role") == "code") Typeface.MONOSPACE else Typeface.DEFAULT
                text.accessibilityLiveRegion = if (kind == "working" || props.optString("role") == "status") {
                    View.ACCESSIBILITY_LIVE_REGION_POLITE
                } else {
                    View.ACCESSIBILITY_LIVE_REGION_NONE
                }
            }
            "rich_text" -> {
                val text = SpannableStringBuilder()
                val runs = props.getJSONArray("runs")
                for (i in 0 until runs.length()) {
                    val run = runs.getJSONObject(i)
                    val start = text.length
                    text.append(run.getString("text"))
                    fun span(value: Any) { text.setSpan(value, start, text.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE) }
                    if (run.optBoolean("bold")) span(StyleSpan(Typeface.BOLD))
                    if (run.optBoolean("italic")) span(StyleSpan(Typeface.ITALIC))
                    if (run.optBoolean("underline")) span(UnderlineSpan())
                    if (run.optBoolean("strike")) span(StrikethroughSpan())
                    run.optJSONObject("foreground")?.let { span(ForegroundColorSpan(color(it))) }
                    run.optJSONObject("background")?.let { span(BackgroundColorSpan(color(it))) }
                }
                (result as TextView).text = text
                result.setTextIsSelectable(true)
                val richTextView = result as TextView
                if (Build.VERSION.SDK_INT >= 28) richTextView.isAccessibilityHeading = props.optString("role") == "heading"
                else richTextView.contentDescription = if (props.optString("role") == "heading") {
                    text.toString().takeIf { it.isNotEmpty() }?.let { "Heading. $it" }
                } else {
                    null
                }
                result.setSingleLine(props.optString("role") == "terminal")
                result.typeface = if (props.optString("role") in setOf("code", "terminal")) Typeface.MONOSPACE else Typeface.DEFAULT
            }
            "dialog" -> {
                val holder = result as LinearLayout
                val open = props.getBoolean("open")
                if (open) {
                    val dialog = dialogs.getOrPut(key) {
                        origins[key] = container?.findFocus()
                        Dialog(context).apply { setContentView(holder) }
                    }
                    dialog.setTitle(props.getString("label"))
                    children(holder, childViews())
                    dialog.setOnCancelListener { event(key) }
                    if (!dialog.isShowing) dialog.show()
                } else {
                    dialogs.remove(key)?.apply { setOnCancelListener(null); dismiss() }
                    origins.remove(key)?.requestFocus()
                    children(holder, emptyList())
                }
                // A modal owns its content in a separate focus scope.
                return View(context).apply { visibility = View.GONE }
            }
            "stack", "list", "message", "tool" -> {
                val layout = if (kind == "list") (result as ScrollView).getChildAt(0) as LinearLayout else result as LinearLayout
                layout.orientation = if (props.optString("axis") == "horizontal") LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
                layout.contentDescription = props.optString("label", "").ifEmpty { null }
                if (node.optJSONObject("style")?.optString("menu") == "context" && nested != null) {
                    children(layout, listOf(node(nested.getJSONObject(0), seen)))
                    layout.setOnLongClickListener {
                        val popup = PopupMenu(context, layout)
                        popups.add(popup)
                        popup.setOnDismissListener { popups.remove(popup) }
                        for (i in 1 until nested.length()) {
                            val item = nested.getJSONObject(i)
                            val p = item.getJSONObject("element").getJSONObject("props")
                            popup.menu.add(p.getString("label")).apply {
                                isEnabled = p.getBoolean("enabled")
                                setOnMenuItemClickListener { event(item.getString("key")); true }
                            }
                        }
                        popup.show(); true
                    }
                } else {
                    layout.setOnLongClickListener(null)
                    val body = childViews()
                    if (kind == "tool") {
                        val heading = TextView(context).apply {
                            text = "${props.getString("name")}: ${props.getString("detail")} — ${props.getString("state")}"
                        }
                        children(layout, listOf(heading) + body)
                    } else children(layout, body)
                }
            }
            else -> if (old != null) {
                val updated = extensions.getValue(kind).mount(node, result)
                require(updated === result) { "An extension must update its existing view." }
            }
        }
        style(result, node.optJSONObject("style") ?: JSONObject())
        return result
    }

    private fun children(parent: ViewGroup, children: List<View>) {
        for (i in parent.childCount - 1 downTo 0) if (parent.getChildAt(i) !in children) parent.removeViewAt(i)
        children.forEachIndexed { index, child ->
            if (parent.indexOfChild(child) != index) {
                (child.parent as? ViewGroup)?.removeView(child)
                val params = if (parent is LinearLayout) LinearLayout.LayoutParams(
                    if (parent.orientation == LinearLayout.HORIZONTAL) -2 else -1, -2
                ) else ViewGroup.LayoutParams(-1, -2)
                parent.addView(child, index, params)
            }
        }
    }
    private fun dp(value: Int) = (context.resources.displayMetrics.density * value).toInt()
    private fun color(value: JSONObject) = Color.argb(value.getInt("alpha"), value.getInt("red"), value.getInt("green"), value.getInt("blue"))
    private fun style(view: View, style: JSONObject) {
        fun space(name: String) = when (style.optString(name)) { "xs" -> 4; "sm" -> 8; "md" -> 16; "lg" -> 24; else -> 0 }
        val padding = style.optJSONArray("padding_points")
        view.setPaddingRelative(dp(padding?.optInt(3) ?: space("padding_start")), dp(padding?.optInt(0) ?: space("padding_top")),
            dp(padding?.optInt(1) ?: space("padding_end")), dp(padding?.optInt(2) ?: space("padding_bottom")))
        view.background = GradientDrawable().apply {
            setColor(style.optJSONObject("background")?.let { color(it) } ?: Color.TRANSPARENT)
            cornerRadius = dp(style.optInt("radius")).toFloat()
            style.optJSONObject("border")?.let { setStroke(dp(1), color(it)) }
        }
        if (view is TextView) {
            style.optJSONObject("foreground")?.let { view.setTextColor(color(it)) }
            view.textSize = style.optInt("text_size", 16).toFloat()
        }
    }
    private fun release(key: String) {
        val control = controls.remove(key) ?: return
        control.view.setOnClickListener(null)
        control.view.setOnLongClickListener(null)
        if (control.kind == "choice") (control.view as ViewGroup).getChildAt(0)?.setOnClickListener(null)
        if (control.view is EditText) {
            control.watcher?.let { control.view.removeTextChangedListener(it) }
            control.view.text.clear()
        }
        dialogs.remove(key)?.apply { setOnCancelListener(null); dismiss() }
        origins.remove(key)?.requestFocus()
        extensions[control.kind]?.dispose(control.view)
        (control.view.parent as? ViewGroup)?.removeView(control.view)
    }
    fun dispose() {
        check(Looper.myLooper() == Looper.getMainLooper())
        disposed = true
        popups.toList().forEach { it.dismiss() }
        popups.clear()
        current = null
        controls.keys.toList().forEach { release(it) }
        container?.removeAllViews()
        container = null
        semantics?.close()
    }
}
