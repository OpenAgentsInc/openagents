package com.openagents.app.rustnative

import android.app.AlertDialog
import android.content.Context
import android.graphics.Color as AndroidColor
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.inputmethod.BaseInputConnection
import android.text.method.PasswordTransformationMethod
import android.text.style.BackgroundColorSpan
import android.text.SpannableStringBuilder
import android.text.style.ForegroundColorSpan
import android.text.style.RelativeSizeSpan
import android.text.style.StrikethroughSpan
import android.text.style.StyleSpan
import android.text.style.UnderlineSpan
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.RadioButton
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import java.io.Closeable
import java.nio.charset.StandardCharsets
import java.util.WeakHashMap
import kotlin.math.roundToInt

/** Sends identity-only events to the Rust view that owns the current intent. */
interface RustNativeViewCallbacks {
    /** Resolve this event with the current `ValidatedView::activate` method. */
    fun activate(event: Activation)

    /** Resolve this event with the current `ValidatedView::change_field` method. */
    fun changeField(event: FieldChange)
}

data class Activation(val instance: String, val revision: Long, val node: String)

data class FieldChange(val activation: Activation, val value: String)

enum class AdapterSupport {
    NATIVE,
    FALLBACK,
    UNSUPPORTED,
}

/**
 * Renders a Rust Native view as Android framework Views.
 *
 * The caller supplies JSON from Rust after `View::validate`. This adapter
 * applies allocation bounds again, but Rust remains the authority for schema
 * validation, intents, and application state. Dimensions named in logical
 * points map to Android dp at a 1:1 scale; Android applies display density.
 */
class AndroidRustNativeViewAdapter(
    context: Context,
    private val callbacks: RustNativeViewCallbacks,
    private val surfaces: Map<String, (Context, String) -> View> = emptyMap(),
) {
    private val context = context
    private val density = context.resources.displayMetrics.density
    private val mounts = WeakHashMap<ViewGroup, MountedView>()

    /** Reports every semantic element this adapter handles or refuses. */
    fun elementSupport(): Map<String, AdapterSupport> = mapOf(
        "stack" to AdapterSupport.NATIVE,
        "list" to AdapterSupport.NATIVE,
        "text" to AdapterSupport.NATIVE,
        "rich_text" to AdapterSupport.NATIVE,
        "field" to AdapterSupport.NATIVE,
        "choice" to AdapterSupport.NATIVE,
        "dialog" to AdapterSupport.NATIVE,
        "button" to AdapterSupport.NATIVE,
        "surface" to AdapterSupport.NATIVE,
        "message" to AdapterSupport.FALLBACK,
        "transcript" to AdapterSupport.UNSUPPORTED,
        "markdown" to AdapterSupport.UNSUPPORTED,
        "tool" to AdapterSupport.UNSUPPORTED,
        "working" to AdapterSupport.UNSUPPORTED,
        "composer" to AdapterSupport.UNSUPPORTED,
    )

    /** Reports style properties that use native rendering or a stated fallback. */
    fun styleSupport(): Map<String, AdapterSupport> = mapOf(
        "foreground" to AdapterSupport.NATIVE,
        "background" to AdapterSupport.NATIVE,
        "padding_top" to AdapterSupport.NATIVE,
        "padding_end" to AdapterSupport.NATIVE,
        "padding_bottom" to AdapterSupport.NATIVE,
        "padding_start" to AdapterSupport.NATIVE,
        "padding_points" to AdapterSupport.NATIVE,
        "gap" to AdapterSupport.NATIVE,
        "gap_points" to AdapterSupport.NATIVE,
        "weight" to AdapterSupport.FALLBACK,
        "align" to AdapterSupport.NATIVE,
        "radius" to AdapterSupport.NATIVE,
        "border" to AdapterSupport.NATIVE,
        "text_size" to AdapterSupport.NATIVE,
        "line_height" to AdapterSupport.NATIVE,
        "min_height" to AdapterSupport.NATIVE,
        "button_padding" to AdapterSupport.NATIVE,
        "monospace" to AdapterSupport.NATIVE,
        "menu" to AdapterSupport.NATIVE,
        "viewport" to AdapterSupport.NATIVE,
        "fill_height" to AdapterSupport.NATIVE,
        "intrinsic_width" to AdapterSupport.NATIVE,
        "icon" to AdapterSupport.FALLBACK,
        "shortcut" to AdapterSupport.NATIVE,
        "button_detail" to AdapterSupport.FALLBACK,
        "button_avatar" to AdapterSupport.FALLBACK,
        "glyph_size" to AdapterSupport.FALLBACK,
        "glyph_gap" to AdapterSupport.FALLBACK,
        "glyph_color" to AdapterSupport.FALLBACK,
        "hover_background" to AdapterSupport.FALLBACK,
        "hover_foreground" to AdapterSupport.FALLBACK,
    )

    /** Mounts a newer view and reuses matching keyed controls when possible. */
    fun mount(json: String, target: ViewGroup): MountedView {
        checkMainThread()
        val view = parseView(json)
        val previous = mounts[target]
        if (previous != null) {
            require(previous.instance == view.instance) {
                "Close the mounted Rust Native view before changing its instance"
            }
            require(view.revision > previous.revision) {
                "Rust Native view revision is not newer than the mounted revision"
            }
        }

        val focusedKey = previous?.focusedKey
        val scrollOffsets = previous?.scrollOffsets.orEmpty()
        val next = MountedView(this, target, view.instance, view.revision, previous, view.root)
        val rendered = try {
            renderNode(view.root, next)
        } catch (error: RuntimeException) {
            next.dispose()
            throw error
        }

        detachFromParent(rendered)
        target.removeAllViews()
        target.addView(rendered, matchParent())
        mounts[target] = next
        previous?.disposeExcept(next.views.values.toSet())

        if (focusedKey != null) next.views[focusedKey]?.requestFocus()
        scrollOffsets.forEach { (key, offset) ->
            (next.views[key] as? ScrollView)?.post { (next.views[key] as? ScrollView)?.scrollTo(0, offset) }
        }
        target.post { next.showDialogs() }
        return next
    }

    private fun parseView(json: String): NativeView {
        val bytes = json.toByteArray(StandardCharsets.UTF_8)
        require(bytes.size <= MAX_VIEW_BYTES) { "Rust Native view exceeds its byte bound" }
        require(jsonDepth(bytes) <= MAX_JSON_DEPTH) { "Rust Native view exceeds its JSON nesting bound" }

        val tokenizer = JSONTokener(json)
        val root = tokenizer.nextValue() as? JSONObject
            ?: throw IllegalArgumentException("Rust Native view must be a JSON object")
        require(tokenizer.nextClean() == '\u0000') { "Rust Native view has trailing JSON data" }
        requireJsonStringsWithinBound(root)
        val schema = root.getString("schema")
        require(schema == SCHEMA_V2 || schema == SCHEMA_V3) { "Unsupported Rust Native view schema" }
        val instance = root.getString("instance")
        require(validId(instance)) { "Invalid Rust Native view instance" }
        val revision = root.getLong("revision")
        require(revision > 0) { "Rust Native view revision must be positive" }

        val keys = HashSet<String>()
        val count = intArrayOf(0)
        val node = parseNode(root.getJSONObject("root"), schema, keys, count, 1)
        return NativeView(instance, revision, node)
    }

    private fun parseNode(
        objectValue: JSONObject,
        schema: String,
        keys: MutableSet<String>,
        count: IntArray,
        depth: Int,
    ): NativeNode {
        require(depth <= MAX_DEPTH) { "Rust Native view exceeds its node depth bound" }
        count[0] += 1
        require(count[0] <= MAX_NODES) { "Rust Native view exceeds its node bound" }

        val key = objectValue.getString("key")
        require(validId(key) && keys.add(key)) { "Invalid or duplicate Rust Native node key" }
        val element = objectValue.getJSONObject("element")
        val kind = element.getString("kind")
        require(elementSupport()[kind] != null) { "Unknown Rust Native element kind: $kind" }
        require(elementSupport()[kind] != AdapterSupport.UNSUPPORTED) {
            "Android Rust Native adapter does not support $kind"
        }
        if (schema == SCHEMA_V2) {
            require(kind !in V3_ELEMENTS) { "Rust Native v2 cannot contain $kind" }
        }

        val props = element.getJSONObject("props")
        val children = when (kind) {
            "stack", "list", "choice", "dialog", "message" ->
                parseChildren(props.optJSONArray("children") ?: JSONArray(), schema, keys, count, depth)
            else -> emptyList()
        }
        if (kind == "stack" && objectValue.optJSONObject("style")?.optString("menu") == "context") {
            require(children.size >= 2 && children.drop(1).all { it.kind == "button" }) {
                "Rust Native context menu requires a card and button items"
            }
        }
        when (kind) {
            "button" -> require(props.getString("label").isNotBlank()) { "Button requires a label" }
            "field" -> {
                require(schema == SCHEMA_V3) { "Field requires Rust Native v3" }
                require(props.getString("label").isNotBlank()) { "Field requires a label" }
                require(props.getInt("max_bytes") in 1..MAX_TEXT_BYTES) { "Field byte bound is invalid" }
                require(!props.getBoolean("secret") || props.getString("value").isEmpty()) {
                    "Secret fields cannot include an initial value"
                }
            }
            "choice", "dialog" -> require(schema == SCHEMA_V3) { "$kind requires Rust Native v3" }
            "surface" -> {
                val resource = props.getString("resource")
                require(resource in surfaces) { "No Android renderer is registered for surface $resource" }
            }
        }
        if (kind == "stack" && props.optString("axis") == "wrap") {
            // The contract permits adapters without wrap to use a vertical stack.
        }

        return NativeNode(key, kind, props, objectValue.optJSONObject("style") ?: JSONObject(), children)
    }

    private fun parseChildren(
        array: JSONArray,
        schema: String,
        keys: MutableSet<String>,
        count: IntArray,
        depth: Int,
    ): List<NativeNode> = buildList(array.length()) {
        for (index in 0 until array.length()) {
            add(parseNode(array.getJSONObject(index), schema, keys, count, depth + 1))
        }
    }

    private fun requireJsonStringsWithinBound(value: Any) {
        when (value) {
            is JSONObject -> {
                val keys = value.keys()
                while (keys.hasNext()) {
                    val key = keys.next()
                    require(key.toByteArray(StandardCharsets.UTF_8).size <= MAX_TEXT_BYTES) {
                        "Rust Native object key exceeds its byte bound"
                    }
                    requireJsonStringsWithinBound(value.get(key))
                }
            }
            is JSONArray -> for (index in 0 until value.length()) requireJsonStringsWithinBound(value.get(index))
            is String -> require(value.toByteArray(StandardCharsets.UTF_8).size <= MAX_TEXT_BYTES) {
                "Rust Native text exceeds its byte bound"
            }
        }
    }

    private fun renderNode(node: NativeNode, mount: MountedView): View {
        return when (node.kind) {
            "stack" -> renderStack(node, mount)
            "list" -> renderList(node, mount)
            "text" -> renderText(node, mount)
            "rich_text" -> renderRichText(node, mount)
            "button" -> renderButton(node, mount)
            "field" -> renderField(node, mount)
            "choice" -> renderChoice(node, mount)
            "dialog" -> renderDialog(node, mount)
            "surface" -> renderSurface(node, mount)
            "message" -> renderMessage(node, mount)
            else -> throw UnsupportedOperationException("Unsupported Rust Native element: ${node.kind}")
        }
    }

    private fun renderStack(node: NativeNode, mount: MountedView): View {
        val axis = node.props.optString("axis", "vertical")
        val stack = linear(node, mount, axis == "horizontal")
        val menu = node.style.optString("menu") == "context"
        val visibleChildren = if (menu) node.children.take(1) else node.children
        addChildren(stack, visibleChildren, node.style, mount)

        val viewport = node.style.optJSONObject("viewport")
        val content: View = if (viewport != null) {
            ScrollView(context).apply {
                isFillViewport = false
                addView(
                    stack,
                    ScrollView.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                    ),
                )
                val maxHeight = dp(viewport.optInt("max_height", 0))
                if (maxHeight > 0) {
                    layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, maxHeight)
                }
                val fade = dp(viewport.optInt("fade", 0))
                if (fade > 0) {
                    isVerticalFadingEdgeEnabled = true
                    setFadingEdgeLength(fade)
                }
                post { scrollTo(0, dp(viewport.optInt("offset", 0))) }
            }
        } else stack

        applyStyle(content, node.style)
        if (!menu) return register(node, mount, content)

        val menuFrame = FrameLayout(context)
        menuFrame.addView(
            content,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ),
        )
        mount.own(menuFrame)
        val menuButton = Button(context).apply {
            text = "⋮"
            contentDescription = "More actions"
            setPadding(dp(12), dp(8), dp(12), dp(8))
            minimumWidth = dp(48)
            minimumHeight = dp(48)
            setOnClickListener { showContextMenu(node, this, mount) }
        }
        mount.own(menuButton)
        menuFrame.addView(
            menuButton,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.END,
            ),
        )
        menuFrame.setOnLongClickListener {
            showContextMenu(node, menuFrame, mount)
            true
        }
        applyStyle(menuFrame, node.style)
        return register(node, mount, menuFrame)
    }

    private fun renderList(node: NativeNode, mount: MountedView): View {
        val rows = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
        mount.own(rows)
        addChildren(rows, node.children, node.style, mount)
        val scroll = obtain(node, mount, ScrollView::class.java) { ScrollView(context) }.apply {
            contentDescription = node.props.getString("label")
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            removeAllViews()
            addView(
                rows,
                ScrollView.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ),
            )
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            scroll.accessibilityClassName = "android.widget.ListView"
        }
        applyStyle(scroll, node.style)
        return scroll
    }

    private fun renderText(node: NativeNode, mount: MountedView): View {
        val value = node.props.getString("value")
        val role = node.props.optString("role", "body")
        val text = obtain(node, mount, TextView::class.java) { TextView(context) }.apply {
            this.text = value
            contentDescription = value
            when (role) {
                "heading" -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) isAccessibilityHeading = true
                "code", "terminal" -> typeface = Typeface.MONOSPACE
                "status" -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT) {
                    accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
                }
            }
            maxLines = if (role == "terminal") 1 else Int.MAX_VALUE
            if (role == "terminal") setHorizontallyScrolling(true)
        }
        applyStyle(text, node.style)
        return text
    }

    private fun renderRichText(node: NativeNode, mount: MountedView): View {
        val runs = node.props.getJSONArray("runs")
        val builder = SpannableStringBuilder()
        for (index in 0 until runs.length()) {
            val run = runs.getJSONObject(index)
            val start = builder.length
            builder.append(run.getString("text"))
            val end = builder.length
            if (end == start) continue
            run.optJSONObject("foreground")?.let {
                builder.setSpan(ForegroundColorSpan(color(it)), start, end, 0)
            }
            run.optJSONObject("background")?.let {
                builder.setSpan(BackgroundColorSpan(color(it)), start, end, 0)
            }
            val typefaceStyle = (if (run.optBoolean("bold")) Typeface.BOLD else 0) or
                (if (run.optBoolean("italic")) Typeface.ITALIC else 0)
            if (typefaceStyle != 0) builder.setSpan(StyleSpan(typefaceStyle), start, end, 0)
            if (run.optBoolean("underline")) builder.setSpan(UnderlineSpan(), start, end, 0)
            if (run.optBoolean("strike")) builder.setSpan(StrikethroughSpan(), start, end, 0)
            if (run.optBoolean("dim")) builder.setSpan(RelativeSizeSpan(0.85f), start, end, 0)
        }
        val text = obtain(node, mount, TextView::class.java) { TextView(context) }.apply {
            this.text = builder
        }
        applyStyle(text, node.style)
        return text
    }

    private fun renderButton(node: NativeNode, mount: MountedView): View {
        val props = node.props
        val label = props.getString("label")
        val shortcut = props.optString("shortcut").takeIf { props.has("shortcut") }
        val glyph = props.optJSONObject("icon")?.optString("glyph")
        val checked = when (glyph) {
            "checked" -> true
            "unchecked" -> false
            else -> null
        }
        val button: View = if (checked != null) {
            obtain(node, mount, CheckBox::class.java) { CheckBox(context) }.apply {
                text = label
                contentDescription = label
                isEnabled = props.optBoolean("enabled", true)
                isFocusable = true
                minimumHeight = maxOf(minimumHeight, dp(48))
                isChecked = checked
                setOnClickListener {
                    isChecked = checked
                    callbacks.activate(mount.activation(node.key))
                }
            }
        } else {
            obtain(node, mount, Button::class.java) { Button(context) }.apply {
                text = if (shortcut.isNullOrBlank()) label else "$label  ·  $shortcut"
                contentDescription = if (shortcut.isNullOrBlank()) label else "$label, shortcut $shortcut"
                isEnabled = props.optBoolean("enabled", true)
                isFocusable = true
                minimumHeight = maxOf(minimumHeight, dp(48))
                setOnClickListener { callbacks.activate(mount.activation(node.key)) }
            }
        }
        applyStyle(button, node.style)
        return button
    }

    private fun renderField(node: NativeNode, mount: MountedView): View {
        val props = node.props
        val secret = props.getBoolean("secret")
        val enabled = props.optBoolean("enabled", true)
        val field = obtain(node, mount, EditText::class.java) { EditText(context) }.apply {
            hint = props.optString("placeholder").ifBlank { props.getString("label") }
            contentDescription = props.getString("label")
            isEnabled = enabled
            isSingleLine = !props.optBoolean("multiline")
            inputType = if (secret) {
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            } else {
                InputType.TYPE_CLASS_TEXT or
                    if (props.optBoolean("multiline")) InputType.TYPE_TEXT_FLAG_MULTI_LINE
                    else InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            }
            if (secret) {
                transformationMethod = PasswordTransformationMethod.getInstance()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
                }
            } else {
                transformationMethod = null
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_AUTO
                }
            }
            maxLines = if (props.optBoolean("multiline")) 6 else 1
            setTextIfChanged(props.getString("value"))
        }
        mount.watchField(node, field, props.getInt("max_bytes"))
        mount.secret(field, secret)
        applyStyle(field, node.style)

        val label = TextView(context).apply {
            text = props.getString("label")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.JELLY_BEAN_MR1) {
                labelFor = field.id.takeIf { it != View.NO_ID } ?: View.generateViewId().also { field.id = it }
            }
        }
        mount.own(label)
        return LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(label, matchWrap())
            addView(field, matchWrap())
            applyStyle(this, node.style)
        }.also { mount.own(it) }
    }

    private fun renderChoice(node: NativeNode, mount: MountedView): View {
        val props = node.props
        val choice = RadioButton(context).apply {
            text = props.getString("label")
            contentDescription = props.getString("label")
            isChecked = props.optBoolean("selected")
            isEnabled = props.optBoolean("enabled", true)
            setOnClickListener {
                isChecked = props.optBoolean("selected")
                callbacks.activate(mount.activation(node.key))
            }
            minimumHeight = maxOf(minimumHeight, dp(48))
        }
        mount.own(choice)
        applyStyle(choice, node.style)
        val row = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(choice, matchWrap())
            addChildren(this, node.children, JSONObject(), mount)
            applyStyle(this, node.style)
        }
        mount.own(row)
        return register(node, mount, row)
    }

    private fun renderDialog(node: NativeNode, mount: MountedView): View {
        val placeholder = obtain(node, mount, FrameLayout::class.java) { FrameLayout(context) }
        if (!node.props.optBoolean("open")) return register(node, mount, placeholder)

        val body = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addChildren(this, node.children, JSONObject(), mount)
            setPadding(dp(20), dp(8), dp(20), dp(8))
        }
        mount.own(body)
        val origin = mount.target.findFocus()
        val dialog = AlertDialog.Builder(context)
            .setTitle(node.props.getString("label"))
            .setView(body)
            .setOnCancelListener { callbacks.activate(mount.activation(node.key)) }
            .create()
        dialog.setOnDismissListener {
            mount.dialogs.removeAll { it.dialog === dialog }
            if (origin?.isAttachedToWindow == true) origin.requestFocus()
        }
        mount.dialogs += ManagedDialog(dialog)
        return register(node, mount, placeholder)
    }

    private fun renderSurface(node: NativeNode, mount: MountedView): View {
        val resource = node.props.getString("resource")
        val label = node.props.getString("label")
        val surface = surfaces.getValue(resource)(context, resource)
        surface.contentDescription = label
        surface.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        applyStyle(surface, node.style)
        return register(node, mount, surface)
    }

    private fun renderMessage(node: NativeNode, mount: MountedView): View {
        val message = linear(node, mount, horizontal = false)
        val role = node.props.optString("role")
        message.contentDescription = when (role) {
            "user" -> "You"
            "assistant" -> "OpenAgents"
            "system" -> "Update"
            else -> ""
        }.ifBlank { null }
        addChildren(message, node.children, node.style, mount)
        return register(node, mount, message)
    }

    private fun linear(node: NativeNode, mount: MountedView, horizontal: Boolean): LinearLayout =
        obtain(node, mount, LinearLayout::class.java) { LinearLayout(context) }.apply {
            orientation = if (horizontal) LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
            removeAllViews()
            applyStyle(this, node.style)
        }

    private fun addChildren(
        parent: LinearLayout,
        children: List<NativeNode>,
        style: JSONObject,
        mount: MountedView,
    ) {
        val gap = style.optInt("gap_points", -1).takeIf { it >= 0 }
            ?: space(style.optString("gap", "none"))
        children.forEachIndexed { index, node ->
            if (index > 0 && gap > 0) {
                val spacer = View(context).also(mount::own)
                val params = if (parent.orientation == LinearLayout.HORIZONTAL) {
                    LinearLayout.LayoutParams(dp(gap), 1)
                } else {
                    LinearLayout.LayoutParams(1, dp(gap))
                }
                parent.addView(spacer, params)
            }
            val child = renderNode(node, mount)
            val params = if (parent.orientation == LinearLayout.HORIZONTAL) {
                LinearLayout.LayoutParams(
                    if (node.style.optBoolean("intrinsic_width")) ViewGroup.LayoutParams.WRAP_CONTENT else 0,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    if (node.style.optBoolean("intrinsic_width")) 0f else 1f,
                )
            } else {
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    if (node.style.optBoolean("fill_height")) 0 else ViewGroup.LayoutParams.WRAP_CONTENT,
                    if (node.style.optBoolean("fill_height")) 1f else 0f,
                )
            }
            detachFromParent(child)
            parent.addView(child, params)
        }
    }

    private fun showContextMenu(node: NativeNode, anchor: View, mount: MountedView) {
        val menu = PopupMenu(context, anchor)
        val actionKeys = node.children.drop(1).map(NativeNode::key)
        node.children.drop(1).forEachIndexed { index, item ->
            menu.menu.add(0, index, index, item.props.getString("label"))
        }
        menu.setOnMenuItemClickListener { item ->
            val key = actionKeys.getOrNull(item.itemId)
            val current = mounts[mount.target]
            val action = key?.let { current?.nodesByKey?.get(it) }
            if (action?.kind == "button" && current != null) {
                callbacks.activate(current.activation(action.key))
                true
            } else false
        }
        menu.show()
    }

    private fun applyStyle(view: View, style: JSONObject) {
        if (style.has("background") || style.has("border")) view.background = background(style)
        val exact = style.optJSONArray("padding_points")
        val top = exact?.optInt(0) ?: space(style.optString("padding_top", "none"))
        val end = exact?.optInt(1) ?: space(style.optString("padding_end", "none"))
        val bottom = exact?.optInt(2) ?: space(style.optString("padding_bottom", "none"))
        val start = exact?.optInt(3) ?: space(style.optString("padding_start", "none"))
        if (top + end + bottom + start > 0) view.setPadding(dp(start), dp(top), dp(end), dp(bottom))
        style.optInt("min_height", 0).takeIf { it > 0 }?.let { view.minimumHeight = dp(it) }

        if (view is TextView) {
            style.optJSONObject("foreground")?.let { view.setTextColor(color(it)) }
            style.optInt("text_size", 0).takeIf { it > 0 }?.let { view.textSize = it.toFloat() }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                style.optInt("line_height", 0).takeIf { it > 0 }?.let { view.setLineHeight(dp(it)) }
            }
            when (style.optString("weight")) {
                "medium" -> view.typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
                "semibold", "bold" -> view.setTypeface(view.typeface, Typeface.BOLD)
            }
            when (style.optString("align")) {
                "center" -> view.gravity = Gravity.CENTER_HORIZONTAL
                "end" -> view.gravity = Gravity.END
                "start" -> view.gravity = Gravity.START
            }
            if (style.optBoolean("monospace")) view.typeface = Typeface.MONOSPACE
            style.optJSONArray("button_padding")?.let {
                view.setPadding(dp(it.optInt(0)), dp(it.optInt(1)), dp(it.optInt(0)), dp(it.optInt(1)))
            }
        }
    }

    private fun background(style: JSONObject): GradientDrawable {
        val drawable = GradientDrawable()
        drawable.setColor(style.optJSONObject("background")?.let(::color) ?: AndroidColor.TRANSPARENT)
        drawable.cornerRadius = dp(style.optInt("radius", 0)).toFloat()
        style.optJSONObject("border")?.let { drawable.setStroke(dp(1), color(it)) }
        return drawable
    }

    private fun color(json: JSONObject): Int = AndroidColor.argb(
        json.optInt("alpha", 255),
        json.getInt("red"),
        json.getInt("green"),
        json.getInt("blue"),
    )

    private fun <T : View> obtain(node: NativeNode, mount: MountedView, type: Class<T>, create: () -> T): T {
        val old = mount.previous?.views?.get(node.key)
        val reusable: T? = old
            ?.takeIf { type.isInstance(it) && (it.tag as? NodeTag)?.kind == node.kind }
            ?.let(type::cast)
        if (reusable != null) mount.previous?.unbind(node.key)
        val view = reusable ?: create()
        view.tag = NodeTag(node.key, node.kind)
        view.isFocusable = view.isFocusable || view is EditText
        mount.views[node.key] = view
        mount.own(view)
        return view
    }

    private fun register(node: NativeNode, mount: MountedView, view: View): View {
        view.tag = NodeTag(node.key, node.kind)
        mount.views[node.key] = view
        mount.own(view)
        return view
    }

    private fun detachFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }

    private fun matchParent() = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
    private fun matchWrap() = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
    private fun dp(points: Int): Int = (points * density).roundToInt()

    private fun space(token: String): Int = when (token) {
        "xs" -> 4
        "sm" -> 8
        "md" -> 16
        "lg" -> 24
        else -> 0
    }

    private fun checkMainThread() {
        check(android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            "Android Rust Native views must mount on the main thread"
        }
    }

    private fun jsonDepth(bytes: ByteArray): Int {
        var depth = 0
        var maximum = 0
        var quoted = false
        var escaped = false
        for (byte in bytes) {
            val char = byte.toInt().toChar()
            if (quoted) {
                if (escaped) escaped = false
                else if (char == '\\') escaped = true
                else if (char == '"') quoted = false
            } else when (char) {
                '"' -> quoted = true
                '{', '[' -> {
                    depth += 1
                    maximum = maxOf(maximum, depth)
                }
                '}', ']' -> depth -= 1
            }
            require(depth >= 0) { "Malformed Rust Native JSON nesting" }
        }
        return maximum
    }

    private fun validId(value: String): Boolean =
        value.length in 1..96 && value.all { it.isLetterOrDigit() || it in "_.:-" } && value.all { it.code < 128 }

    private fun EditText.setTextIfChanged(value: String) {
        if (text.toString() == value) return
        if (BaseInputConnection.getComposingSpanStart(text) >= 0) return
        val start = selectionStart.coerceAtLeast(0)
        val end = selectionEnd.coerceAtLeast(0)
        setText(value)
        setSelection(start.coerceAtMost(text.length), end.coerceAtMost(text.length))
    }

    internal data class NativeView(val instance: String, val revision: Long, val root: NativeNode)
    internal data class NativeNode(
        val key: String,
        val kind: String,
        val props: JSONObject,
        val style: JSONObject,
        val children: List<NativeNode>,
    )
    private data class NodeTag(val key: String, val kind: String)
    internal data class ManagedDialog(val dialog: AlertDialog)

    inner class MountedView internal constructor(
        private val adapter: AndroidRustNativeViewAdapter,
        internal val target: ViewGroup,
        internal val instance: String,
        internal val revision: Long,
        internal val previous: MountedView?,
        root: NativeNode,
    ) : Closeable {
        internal val nodesByKey: Map<String, NativeNode> = buildMap {
            fun index(node: NativeNode) {
                put(node.key, node)
                node.children.forEach(::index)
            }
            index(root)
        }
        internal val views = LinkedHashMap<String, View>()
        internal val dialogs = ArrayList<ManagedDialog>()
        private val ownedViews = LinkedHashSet<View>()
        private val watchers = LinkedHashMap<String, Pair<EditText, TextWatcher>>()
        private val secrets = LinkedHashSet<EditText>()
        private var disposed = false
        internal val focusedKey: String? get() = views.entries.firstOrNull { it.value.hasFocus() }?.key
        internal val scrollOffsets: Map<String, Int>
            get() = views.mapNotNull { (key, view) ->
                (view as? ScrollView)?.let { key to it.scrollY }
            }.toMap()

        internal fun activation(node: String) = Activation(instance, revision, node)

        internal fun watchField(node: NativeNode, field: EditText, maxBytes: Int) {
            val previousWatcher = watchers.remove(node.key)
            if (previousWatcher != null) previousWatcher.first.removeTextChangedListener(previousWatcher.second)
            val watcher = object : TextWatcher {
                private var applying = false
                private var previousValue = field.text.toString()
                override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {
                    if (!applying) previousValue = s?.toString().orEmpty()
                }
                override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
                override fun afterTextChanged(editable: Editable?) {
                    if (applying || editable == null) return
                    if (BaseInputConnection.getComposingSpanStart(editable) >= 0) return
                    val value = editable.toString()
                    if (value.toByteArray(StandardCharsets.UTF_8).size > maxBytes ||
                        (!node.props.optBoolean("multiline") && value.any { it == '\n' || it == '\r' })
                    ) {
                        applying = true
                        field.setText(previousValue)
                        field.setSelection(field.text.length)
                        applying = false
                        return
                    }
                    callbacks.changeField(FieldChange(activation(node.key), value))
                }
            }
            field.addTextChangedListener(watcher)
            watchers[node.key] = field to watcher
        }

        internal fun secret(field: EditText, secret: Boolean) {
            if (secret) secrets += field
        }

        internal fun own(view: View) {
            ownedViews += view
        }

        internal fun unbind(key: String) {
            val view = views[key] ?: return
            view.setOnClickListener(null)
            view.setOnLongClickListener(null)
            val watcher = watchers.remove(key)
            if (watcher != null) watcher.first.removeTextChangedListener(watcher.second)
        }

        internal fun showDialogs() {
            if (disposed) return
            dialogs.forEach { if (!it.dialog.isShowing) it.dialog.show() }
        }

        internal fun disposeExcept(retained: Set<View>) {
            if (disposed) return
            disposed = true
            watchers.values.forEach { (field, watcher) -> field.removeTextChangedListener(watcher) }
            watchers.clear()
            ownedViews.filterNot { it in retained }.forEach { view ->
                view.setOnClickListener(null)
                view.setOnLongClickListener(null)
                (view as? EditText)?.let { if (it in secrets) it.setText("") }
            }
            secrets.filterNot { it in retained }.forEach { it.setText("") }
            dialogs.forEach { managed ->
                managed.dialog.setOnCancelListener(null)
                managed.dialog.setOnDismissListener(null)
                managed.dialog.dismiss()
            }
            dialogs.clear()
            if (adapter.mounts[target] === this) adapter.mounts.remove(target)
        }

        override fun close() {
            adapter.checkMainThread()
            val isCurrent = adapter.mounts[target] === this
            disposeExcept(emptySet())
            if (isCurrent) target.removeAllViews()
        }
    }

    private fun MountedView.dispose() = disposeExcept(emptySet())

    private companion object {
        const val SCHEMA_V2 = "rust-native.view.v2"
        const val SCHEMA_V3 = "rust-native.view.v3"
        const val MAX_VIEW_BYTES = 512 * 1024
        const val MAX_NODES = 1_024
        const val MAX_DEPTH = 16
        const val MAX_JSON_DEPTH = 96
        const val MAX_TEXT_BYTES = 64 * 1024
        val V3_ELEMENTS = setOf("field", "rich_text", "choice", "dialog")
    }
}
