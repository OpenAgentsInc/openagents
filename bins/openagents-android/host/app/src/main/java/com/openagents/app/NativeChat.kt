// Native conversation elements for Rust Native: message, Markdown, tool,
// working, and composer, as they appear outside a transcript; the transcript
// itself is painted from Rust's layout (TranscriptPainter.kt). Rust decides
// what each row says; this file only paints, scrolls, and handles gestures. It follows the iOS
// renderer's design (bins/coder-ios/host/App/NativeChat.swift), which
// reimplements the t3code iOS chat (pingdotgg/t3code, MIT).
package com.openagents.app

import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Typeface
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.TextUtils
import android.text.TextWatcher
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import android.text.style.StrikethroughSpan
import android.text.style.StyleSpan
import android.text.style.TypefaceSpan
import android.text.style.UnderlineSpan
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.TableLayout
import android.widget.TableRow
import android.widget.TextView
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject

/** Plain text for copying, matching `rust_native::markdown::plain`. */
internal object PlainText {
    fun of(node: JSONObject): String {
        val element = node.getJSONObject("element")
        val props = element.getJSONObject("props")
        return when (element.getString("kind")) {
            "text" -> props.getString("value")
            "markdown" -> blocks(props.getJSONArray("blocks"))
            "button", "working" -> props.getString("label")
            "tool" -> (listOf(props.getString("detail").let { if (it.isEmpty()) props.getString("name") else "${props.getString("name")} $it" }) +
                props.getJSONArray("children").objects().map { of(it) }).joined("\n")
            "stack", "list", "message", "transcript" ->
                props.getJSONArray("children").objects().map { of(it) }.filter { it.isNotEmpty() }.joined("\n\n")
            else -> ""
        }
    }

    fun blocks(blocks: JSONArray): String = blocks.objects().flatMap { block ->
        when (block.getString("kind")) {
            "heading", "paragraph" -> listOf(spans(block.getJSONArray("spans")))
            "list" -> block.getJSONArray("items").objects().mapIndexed { index, item ->
                val marker = when {
                    item.has("checked") && !item.isNull("checked") -> if (item.getBoolean("checked")) "[x] " else "[ ] "
                    block.getBoolean("ordered") -> "${block.getLong("start") + index}. "
                    else -> "- "
                }
                marker + blocks(item.getJSONArray("blocks")).replace("\n", " ")
            }
            "code" -> listOf(block.getString("text").trimEnd('\n'))
            "quote" -> listOf(blocks(block.getJSONArray("blocks")).lines().joinToString("\n") { "> $it" })
            "table" -> (listOf(block.getJSONArray("header")) + block.getJSONArray("rows").let { rows ->
                (0 until rows.length()).map { rows.getJSONArray(it) } }).map { cells ->
                (0 until cells.length()).joinToString(" | ") { spans(cells.getJSONArray(it)) }
            }
            else -> listOf("---")
        }
    }.joined("\n")

    fun spans(spans: JSONArray) = spans.objects().joinToString("") { it.getString("text") }
    private fun List<String>.joined(separator: String) = joinToString(separator)
}

/**
 * Builds conversation rows as plain Android views. Rows inside a transcript
 * are rebuilt only when their content changes; a tool row's expansion is
 * adapter state that survives rebuilds.
 */
class ChatViews(
    private val context: Context,
    private val activate: (String) -> Unit,
    private val submit: ((String, String) -> Unit)?,
) {
    companion object {
        /** Which tool rows the reader expanded, by node key. */
        val expanded = HashSet<String>()
    }

    fun build(node: JSONObject, depth: Int = 0, tone: Int = Palette.PRIMARY): View {
        require(depth <= 16) { "The native view exceeds its size limit." }
        val key = node.getString("key")
        val element = node.getJSONObject("element")
        val props = element.getJSONObject("props")
        val style = node.optJSONObject("style") ?: JSONObject()
        val color = style.objectOrNull("foreground")?.let { NativeRenderer.color(it) } ?: tone
        val view: View = when (element.getString("kind")) {
            "message" -> message(node, props, depth, color)
            "markdown" -> markdown(props.getJSONArray("blocks"), color)
            "tool" -> tool(key, props, depth)
            "working" -> working(props.getString("label"))
            "stack" -> context.column().apply {
                if (props.getString("axis") == "horizontal") orientation = LinearLayout.HORIZONTAL
                props.getJSONArray("children").objects().forEachIndexed { index, child ->
                    addView(build(child, depth + 1, color), LinearLayout.LayoutParams(-1, -2).apply {
                        if (index > 0) topMargin = context.dp(6)
                    })
                }
            }
            "list" -> context.column().apply {
                props.getJSONArray("children").objects().forEach { addView(build(it, depth + 1, color)) }
            }
            "text" -> context.text(props.getString("value"), when (props.getString("role")) {
                "heading" -> 18f; "status" -> 13f; "code" -> 13f; else -> 15f
            }, color).apply {
                setTextIsSelectable(true)
                when (props.getString("role")) {
                    "code", "terminal" -> typeface = Fonts.code(context)
                    "heading" -> typeface = Fonts.typeface(context, Fonts.BOLD)
                }
            }
            "button" -> context.text(props.getString("label"), 15f).apply {
                background = context.rounded(Palette.RAISED, 10f)
                setPadding(context.dp(14), context.dp(10), context.dp(14), context.dp(10))
                isEnabled = props.getBoolean("enabled")
                setOnClickListener { activate(key) }
            }
            else -> context.text("This device can't display this content.", 14f, Palette.SECONDARY)
        }
        view.tag = key
        return view
    }

    // Message

    private fun message(node: JSONObject, props: JSONObject, depth: Int, color: Int): View {
        val role = props.getString("role")
        val note = props.textOrNull("note")
        val children = props.getJSONArray("children").objects()
        val copy = PlainText.of(node)
        val content = context.column().apply {
            children.forEachIndexed { index, child ->
                addView(build(child, depth + 1, if (role == "system") Palette.SECONDARY else color),
                    LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = context.dp(if (role == "user") 8 else 10) })
            }
        }
        val outer = context.column()
        when (role) {
            "user" -> {
                content.background = context.rounded(Palette.BUBBLE, 0f, radii = floatArrayOf(18f, 18f, 4f, 18f))
                content.setPadding(context.dp(14), context.dp(10), context.dp(14), context.dp(10))
                outer.gravity = Gravity.END
                outer.addView(content, LinearLayout.LayoutParams(-2, -2).apply {
                    gravity = Gravity.END; marginStart = context.dp(48)
                })
                note?.let { outer.addView(context.text(it, 12f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply {
                    gravity = Gravity.END; topMargin = context.dp(4) }) }
            }
            "system" -> {
                // A quiet centered row: the text, then " · note".
                val line = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
                content.gravity = Gravity.CENTER_HORIZONTAL
                line.addView(content, LinearLayout.LayoutParams(-2, -2))
                note?.let { line.addView(context.text("· $it", 13f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply {
                    marginStart = context.dp(6) }) }
                outer.addView(line, LinearLayout.LayoutParams(-2, -2).apply { gravity = Gravity.CENTER_HORIZONTAL })
            }
            else -> {
                outer.addView(content, LinearLayout.LayoutParams(-1, -2))
                note?.let { outer.addView(context.text(it, 12f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply {
                    topMargin = context.dp(4) }) }
            }
        }
        outer.setOnLongClickListener { copyText(copy, "Message copied"); true }
        outer.contentDescription = null
        return outer
    }

    private fun copyText(text: String, done: String) {
        context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("OpenAgents", text))
        Toast.makeText(context, done, Toast.LENGTH_SHORT).show()
    }

    // Markdown

    fun markdown(blocks: JSONArray, color: Int): View = context.column().apply {
        blocks.objects().forEachIndexed { index, block ->
            addView(block(block, color), LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = context.dp(10) })
        }
    }

    private fun block(block: JSONObject, color: Int): View = when (block.getString("kind")) {
        "heading" -> {
            val level = block.getInt("level")
            paragraph(block.getJSONArray("spans"), color, when (level) { 1 -> 22f; 2 -> 19f; 3 -> 17f; else -> 16f }).apply {
                typeface = Fonts.typeface(context, Fonts.BOLD)
                setPadding(0, context.dp(if (level <= 2) 4 else 2), 0, 0)
                if (android.os.Build.VERSION.SDK_INT >= 28) isAccessibilityHeading = true
            }
        }
        "paragraph" -> paragraph(block.getJSONArray("spans"), color, 16f)
        "list" -> list(block, color)
        "code" -> code(block.textOrNull("language"), block.getString("text"))
        "quote" -> LinearLayout(context).apply {
            val bar = View(context).apply { background = context.rounded(Palette.BORDER, 1.5f) }
            addView(bar, LinearLayout.LayoutParams(context.dp(3), -1))
            addView(markdown(block.getJSONArray("blocks"), (color and 0x00FFFFFF) or (0xB3 shl 24)),
                LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = context.dp(9) })
        }
        "table" -> table(block, color)
        else -> View(context).apply {
            setBackgroundColor(Palette.BORDER)
            layoutParams = LinearLayout.LayoutParams(-1, context.dp(1))
            minimumHeight = context.dp(1)
        }
    }

    private fun paragraph(spans: JSONArray, color: Int, size: Float): TextView =
        context.text("", size, color).apply {
            text = styled(spans)
            setTextIsSelectable(true)
            // Links stay inert: they are styled, never opened.
            linksClickable = false
            autoLinkMask = 0
            setLineSpacing(context.dpf(2f), 1f)
        }

    fun styled(spans: JSONArray): CharSequence {
        val builder = SpannableStringBuilder()
        for (span in spans.objects()) {
            val start = builder.length
            builder.append(span.getString("text"))
            val end = builder.length
            if (start == end) continue
            fun mark(what: Any) = builder.setSpan(what, start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            if (span.optBoolean("bold")) mark(StyleSpan(Typeface.BOLD))
            if (span.optBoolean("italic")) mark(StyleSpan(Typeface.ITALIC))
            if (span.optBoolean("strike")) mark(StrikethroughSpan())
            if (span.optBoolean("code")) { mark(BackgroundColorSpan(Palette.INLINE_CODE)); mark(TypefaceSpan("monospace")) }
            if (span.textOrNull("link") != null || span.has("link")) { mark(ForegroundColorSpan(Palette.LINK)); mark(UnderlineSpan()) }
        }
        return builder
    }

    private fun list(block: JSONObject, color: Int): View = context.column().apply {
        val ordered = block.getBoolean("ordered")
        val start = block.getLong("start")
        block.getJSONArray("items").objects().forEachIndexed { index, item ->
            val row = context.row()
            val checked = if (item.has("checked") && !item.isNull("checked")) item.getBoolean("checked") else null
            val marker = when {
                checked == true -> "☑"; checked == false -> "☐"
                ordered -> "${start + index}."; else -> "•"
            }
            row.addView(context.text(marker, 16f, if (checked != null) Palette.SECONDARY else color).apply {
                gravity = Gravity.END
                if (ordered && checked == null) typeface = Fonts.typeface(context)
            }, LinearLayout.LayoutParams(context.dp(if (ordered) 28 else 18), -2))
            row.addView(markdown(item.getJSONArray("blocks"), color), LinearLayout.LayoutParams(0, -2, 1f).apply {
                marginStart = context.dp(8) })
            addView(row, LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = context.dp(4) })
        }
    }

    private fun code(language: String?, text: String): View = context.column().apply {
        background = context.rounded(Palette.SURFACE, 10f, Palette.BORDER)
        clipToOutline = true
        val header = context.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(12), context.dp(6), context.dp(6), context.dp(6))
            addView(context.text(language ?: "code", 12f, Palette.SECONDARY), LinearLayout.LayoutParams(0, -2, 1f))
            addView(context.text("Copy", 12f, Palette.SECONDARY).apply {
                setPadding(context.dp(10), context.dp(6), context.dp(10), context.dp(6))
                contentDescription = "Copy code"
                setOnClickListener { copyText(text.trimEnd('\n'), "Code copied") }
            })
        }
        addView(header)
        addView(View(context).apply { setBackgroundColor(Palette.BORDER) }, LinearLayout.LayoutParams(-1, context.dp(1)))
        addView(HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(context.text(text.trimEnd('\n'), 13f).apply {
                typeface = Fonts.code(context)
                setTextIsSelectable(true)
                setHorizontallyScrolling(true)
                setPadding(context.dp(12), context.dp(12), context.dp(12), context.dp(12))
            })
        })
    }

    private fun table(block: JSONObject, color: Int): View {
        val align = block.getJSONArray("align")
        val table = TableLayout(context).apply {
            background = context.rounded(Palette.SURFACE, 8f, Palette.BORDER)
            clipToOutline = true
        }
        fun cells(values: JSONArray, header: Boolean): TableRow = TableRow(context).apply {
            if (header) setBackgroundColor(Palette.RAISED)
            for (column in 0 until values.length()) {
                addView(context.text("", 14f, color).apply {
                    text = styled(values.getJSONArray(column))
                    if (header) typeface = Fonts.typeface(context, Fonts.BOLD)
                    setTextIsSelectable(true)
                    maxWidth = context.dp(280)
                    gravity = when (align.optString(column)) {
                        "center" -> Gravity.CENTER_HORIZONTAL; "right" -> Gravity.END; else -> Gravity.START
                    }
                    setPadding(context.dp(10), context.dp(7), context.dp(10), context.dp(7))
                })
            }
        }
        table.addView(cells(block.getJSONArray("header"), true))
        val rows = block.getJSONArray("rows")
        for (index in 0 until rows.length()) {
            table.addView(View(context).apply { setBackgroundColor(Palette.BORDER) }, TableLayout.LayoutParams(-1, context.dp(1)))
            table.addView(cells(rows.getJSONArray(index), false))
        }
        return HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(table)
        }
    }

    // Tool

    private fun tool(key: String, props: JSONObject, depth: Int): View {
        val name = props.getString("name")
        val detail = props.getString("detail")
        val state = props.getString("state")
        val children = props.getJSONArray("children").objects()
        val outer = context.column()
        val chevron = context.text("", 14f, Palette.TERTIARY)
        val body = context.column().apply {
            background = context.rounded(Palette.SURFACE, 8f)
            setPadding(context.dp(10), context.dp(10), context.dp(10), context.dp(10))
            children.forEachIndexed { index, child ->
                addView(build(child, depth + 1, Palette.PRIMARY).also { smaller(it) },
                    LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = context.dp(6) })
            }
        }
        fun show() {
            val open = key in expanded
            body.visibility = if (open) View.VISIBLE else View.GONE
            chevron.text = if (children.isEmpty()) "" else if (open) "⌄" else "›"
            outer.getChildAt(0)?.stateDescriptionCompat(if (open) "Expanded" else "Collapsed")
        }
        val header = context.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = context.dp(36)
            val icon: View = when (state) {
                "running" -> ProgressBar(context).apply { isIndeterminate = true }
                "failed" -> context.text("✕", 13f, Palette.FAILURE).apply { gravity = Gravity.CENTER }
                else -> context.text("✓", 13f, Palette.SUCCESS).apply { gravity = Gravity.CENTER }
            }
            addView(icon, LinearLayout.LayoutParams(context.dp(18), context.dp(18)))
            addView(context.text(name, 15f).apply { typeface = Fonts.typeface(context, Fonts.BOLD) },
                LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(8) })
            addView(context.text(detail, 15f, Palette.SECONDARY).apply {
                maxLines = 1; ellipsize = TextUtils.TruncateAt.END
            }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = context.dp(8) })
            addView(chevron, LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(4) })
            contentDescription = "$name, ${when (state) { "running" -> "running"; "failed" -> "failed"; else -> "done" }}. $detail"
            isEnabled = children.isNotEmpty()
            setOnClickListener { if (!expanded.remove(key)) expanded.add(key); show() }
        }
        outer.addView(header)
        outer.addView(body, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(4) })
        show()
        return outer
    }

    /** Tool output is secondary: draw it a step smaller than the conversation. */
    private fun smaller(view: View) {
        if (view is TextView) view.setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX,
            (view.textSize - sp(2f)).coerceAtLeast(sp(11f)))
        if (view is ViewGroup) for (index in 0 until view.childCount) smaller(view.getChildAt(index))
    }

    private fun sp(value: Float) = android.util.TypedValue.applyDimension(
        android.util.TypedValue.COMPLEX_UNIT_SP, value, context.resources.displayMetrics)

    // Working

    private fun working(label: String): View = context.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, context.dp(4), 0, context.dp(4))
        for (index in 0 until 3) {
            addView(View(context).apply {
                background = context.rounded(Palette.SECONDARY, 3f)
                ObjectAnimator.ofFloat(this, View.ALPHA, 0.25f, 1f).apply {
                    duration = 600; startDelay = index * 200L
                    repeatMode = ValueAnimator.REVERSE; repeatCount = ValueAnimator.INFINITE
                    start()
                }
            }, LinearLayout.LayoutParams(context.dp(6), context.dp(6)).apply { if (index > 0) marginStart = context.dp(4) })
        }
        addView(context.text(label, 15f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(10) })
        contentDescription = label
    }
}

private fun View.stateDescriptionCompat(value: String) {
    if (android.os.Build.VERSION.SDK_INT >= 30) stateDescription = value
}

/**
 * The composer's text field: it reports selection changes, the delete key,
 * and undo and redo (Ctrl+Z, Ctrl+Shift+Z, and the text menu) to its
 * [Composer], which edits through the shared draft.
 */
class ComposerField(context: Context) : EditText(context) {
    var selectionChanged: ((Int, Int) -> Unit)? = null
    /** Handles the delete key; false lets the field delete, as while an IME composes. */
    var deleteKey: (() -> Boolean)? = null
    var undoKey: ((Boolean) -> Boolean)? = null

    override fun onSelectionChanged(selStart: Int, selEnd: Int) {
        super.onSelectionChanged(selStart, selEnd)
        selectionChanged?.invoke(selStart, selEnd)
    }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean {
        if (keyCode == android.view.KeyEvent.KEYCODE_DEL && deleteKey?.invoke() == true) return true
        return super.onKeyDown(keyCode, event)
    }

    override fun onTextContextMenuItem(id: Int): Boolean {
        // Text only (#10093): a clipboard holding only an image pastes nothing.
        if ((id == android.R.id.paste || id == android.R.id.pasteAsPlainText) && !clipboardHasText()) return true
        if (id == android.R.id.undo && undoKey?.invoke(false) == true) return true
        if (id == android.R.id.redo && undoKey?.invoke(true) == true) return true
        return super.onTextContextMenuItem(id)
    }

    private fun clipboardHasText(): Boolean {
        val clip = context.getSystemService(android.content.ClipboardManager::class.java)?.primaryClip ?: return true
        return (0 until clip.itemCount).any { index ->
            clip.getItemAt(index).let { PhoneAttachments.accepts(it.text, it.uri != null) }
        }
    }

    /** Text only (#10093): a paste, drop, or keyboard insert keeps its text items and drops images and files. */
    @androidx.annotation.RequiresApi(31)
    override fun onReceiveContent(payload: android.view.ContentInfo): android.view.ContentInfo? {
        val clip = payload.clip
        val kept = (0 until clip.itemCount).map { clip.getItemAt(it) }
            .filter { PhoneAttachments.accepts(it.text, it.uri != null) }
        if (kept.isEmpty()) return null
        val text = android.content.ClipData(clip.description, kept[0]).apply { kept.drop(1).forEach { addItem(it) } }
        return super.onReceiveContent(android.view.ContentInfo.Builder(payload).setClip(text).build())
    }

    override fun onKeyShortcut(keyCode: Int, event: android.view.KeyEvent): Boolean {
        if (keyCode == android.view.KeyEvent.KEYCODE_Z && event.isCtrlPressed &&
            undoKey?.invoke(event.isShiftPressed) == true) return true
        return super.onKeyShortcut(keyCode, event)
    }
}

/**
 * A multi-line text field with a send control. A send is an input answer
 * bound to the composer's token; while Rust reports `busy`, the control
 * becomes stop, which activates the composer node. With `choices`, a long
 * press on send offers the other ways to send, each answering with its own
 * token. The draft is Rust Native's shared editor's ([DraftEditor]): the
 * field reports each change and shows the draft Rust returns, a new token
 * starts a new draft (its `draft`, or empty), and a refused send keeps it.
 */
class Composer(private val context: Context, private val send: (String, String) -> Unit,
               private val stop: (String) -> Unit, private val editor: DraftEditor = DraftEditor()) {
    val root = context.row()
    private val field = ComposerField(context)
    /** The field is showing Rust's draft: its own callbacks are echoes. */
    private var showing = false
    private val control = context.text("↑", 17f, Palette.BACKGROUND)
    private var token = ""
    private var maxBytes = 65_536
    private var enabled = true
    private var busy = false
    private var stoppable = false
    /** Each choice's token and label, in order. */
    private var choices: List<Pair<String, String>> = emptyList()

    /** One capsule holding the field and, at its trailing end, the send control. */
    private val capsule = context.row()

    init {
        root.gravity = Gravity.BOTTOM
        root.setPadding(context.dp(12), context.dp(8), context.dp(12), context.dp(8))
        capsule.gravity = Gravity.BOTTOM
        capsule.minimumHeight = context.dp(50)
        capsule.background = context.rounded(Palette.RAISED, 25f, Palette.BORDER)
        capsule.setPadding(context.dp(6), context.dp(8), context.dp(8), context.dp(8))
        field.apply {
            textSize = 16f
            setTextColor(Palette.PRIMARY)
            setHintTextColor(Palette.TERTIARY)
            background = null
            setPadding(context.dp(12), context.dp(6), context.dp(8), context.dp(6))
            minLines = 1; maxLines = 6
            isVerticalScrollBarEnabled = true
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            tag = "composer-field"
            // Refuse an edit past the byte bound, but always allow deleting.
            filters = arrayOf(InputFilter { source, start, end, dest, dstart, dend ->
                val next = dest.substring(0, dstart) + source.subSequence(start, end) + dest.substring(dend)
                if (end > start && next.toByteArray().size > maxBytes) "" else null
            })
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
                override fun afterTextChanged(s: Editable?) { synced(); refresh() }
            })
            selectionChanged = { start, end -> selected(start, end) }
            deleteKey = { deleteBackward() }
            undoKey = { redo -> undo(redo) }
        }
        control.apply {
            gravity = Gravity.CENTER
            typeface = Fonts.typeface(context, Fonts.BOLD)
            background = context.rounded(Palette.PRIMARY, 17f)
            setOnClickListener { if (busy) doStop() else doSend() }
            setOnLongClickListener { offerChoices() }
        }
        capsule.addView(field, LinearLayout.LayoutParams(0, -2, 1f))
        capsule.addView(control, LinearLayout.LayoutParams(context.dp(34), context.dp(34)).apply { marginStart = context.dp(4) })
        root.addView(capsule, LinearLayout.LayoutParams(-1, -2))
    }

    fun update(props: JSONObject) {
        val next = props.getString("token")
        val draft = if (props.has("draft") && !props.isNull("draft")) props.getString("draft") else null
        if (editor.live) {
            if (editor.mount(next, props.getInt("max_bytes"), draft)) editor.state?.let { show(it) }
        } else if (next != token && draft != null) {
            field.setText(draft)
            field.setSelection(field.text.length)
        }
        // A screen whose purpose is to write, such as a new chat, opens
        // with the cursor in the field, once per token.
        val focus = next != token && props.optBoolean("focus") && props.getBoolean("enabled")
        token = next
        choices = props.optJSONArray("choices")?.let { list ->
            (0 until list.length()).map { list.getJSONObject(it).let { c -> c.getString("token") to c.getString("label") } }
        } ?: emptyList()
        maxBytes = props.getInt("max_bytes").coerceIn(1, 65_536)
        enabled = props.getBoolean("enabled")
        busy = props.getBoolean("busy")
        stoppable = props.has("stop") && !props.isNull("stop")
        val placeholder = props.getString("placeholder")
        if (field.hint?.toString() != placeholder) { field.hint = placeholder; field.contentDescription = placeholder }
        field.isEnabled = enabled
        root.alpha = if (enabled) 1f else 0.6f
        refresh()
        if (focus) field.post {
            if (field.requestFocus()) context.getSystemService(android.view.inputmethod.InputMethodManager::class.java)
                ?.showSoftInput(field, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT)
        }
    }

    private val canSend get() = enabled && !busy && field.text.toString().isNotBlank() &&
        field.text.toString().toByteArray().size <= maxBytes

    private fun refresh() {
        // A busy composer without a stop intent shows the send arrow,
        // disabled, never a stop that does nothing.
        val stops = busy && stoppable
        val active = stops || canSend
        control.text = if (stops) "■" else "↑"
        control.contentDescription = if (stops) "Stop" else "Send"
        control.isLongClickable = !busy && choices.isNotEmpty()
        control.tag = if (stops) "composer-stop" else "composer-send"
        control.isEnabled = active
        control.alpha = if (active) 1f else 0.3f
    }

    private fun doSend() = doSend(token)

    private fun doSend(answer: String) {
        if (!canSend) return
        val value = field.text.toString()
        // The shared draft stays until Rust accepts it: the next composer's
        // new token clears it, and a refused send keeps the words.
        if (!editor.live) field.setText("")
        send(answer, value)
    }

    /** Frees the shared draft when the renderer drops this composer. */
    fun close() = editor.close()

    /** Show the shared draft: its text and selection. */
    private fun show(state: DraftEditor.State) {
        showing = true
        try {
            if (field.text.toString() != state.text) field.setText(state.text)
            val length = field.text.length
            if (state.end <= length && (field.selectionStart != state.start || field.selectionEnd != state.end)) {
                field.setSelection(state.start, state.end)
            }
        } finally { showing = false }
    }

    private fun composing(): Pair<Int, Int>? {
        val text = field.text ?: return null
        val start = android.view.inputmethod.BaseInputConnection.getComposingSpanStart(text)
        val end = android.view.inputmethod.BaseInputConnection.getComposingSpanEnd(text)
        return if (start in 0 until end) start to end else null
    }

    /** The field changed: report it; outside a composition, show Rust's draft if it differs. */
    private fun synced() {
        if (showing || !editor.live || editor.state == null) return
        val marked = composing()
        val state = editor.sync(field.text.toString(), field.selectionStart.coerceAtLeast(0),
            field.selectionEnd.coerceAtLeast(0), marked) ?: return
        // Rust refused the change or kept a grapheme whole: show its draft.
        if (marked == null && state.text != field.text.toString()) field.post { if (composing() == null) show(state) }
    }

    private fun selected(start: Int, end: Int) {
        if (showing || !editor.live || composing() != null) return
        val state = editor.state ?: return
        if (state.text != field.text.toString() || (state.start == start && state.end == end)) return
        editor.select(start, end)
    }

    /** The delete key outside a composition: one whole grapheme, or the selection. */
    private fun deleteBackward(): Boolean {
        if (!editor.live || !enabled || composing() != null || editor.state == null) return false
        editor.delete(true)?.let { show(it); refresh() }
        return true
    }

    private fun undo(redo: Boolean): Boolean {
        if (!editor.live || editor.state == null) return false
        (if (redo) editor.redo() else editor.undo())?.let { show(it); refresh() }
        return true
    }

    /** A long press on send: a menu of the other ways to send. */
    private fun offerChoices(): Boolean {
        if (busy || choices.isEmpty() || !canSend) return false
        val menu = PopupMenu(context, control)
        choices.forEachIndexed { index, (_, label) -> menu.menu.add(0, index, index, label) }
        menu.setOnMenuItemClickListener { item ->
            choices.getOrNull(item.itemId)?.let { (choice, _) -> doSend(choice) }
            true
        }
        menu.show()
        return true
    }

    private fun doStop() {
        if (!busy || !stoppable) return
        (root.getTag(R.id.native_key) as? String)?.let { stop(it) }
    }
}
