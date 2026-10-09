package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.ProgressBar
import org.json.JSONObject

/**
 * A host terminal, full screen. Rust owns the session, the emulator, and
 * every byte sent; this screen reports the grid it fits, forwards keys, and
 * polls for output while it shows.
 */
class TerminalScreen(private val activity: MainActivity, private val bridge: MobileBridge) {
    val root = FrameLayout(activity).apply {
        setBackgroundColor(Noir.BACKGROUND)
        isClickable = true
        visibility = View.GONE
        tag = "terminal-screen"
    }
    private val content = FrameLayout(activity)
    private val progress = ProgressBar(activity)
    private val renderer = NativeRenderer(activity, { view, node -> bridge.activate("terminal", view, node) })
    private val keys = TerminalKeyView(activity,
        text = { bridge.terminal(json("op" to "terminal_text", "text" to it)) },
        key = { name, ctrl, alt, shift ->
            bridge.terminal(json("op" to "terminal_key", "key" to name, "ctrl" to ctrl, "alt" to alt, "shift" to shift))
        })
    private val main = Handler(Looper.getMainLooper())
    private var grid = 0 to 0
    private var shown = false
    private val poll = object : Runnable {
        override fun run() {
            if (!shown) return
            bridge.pollTerminal()
            main.postDelayed(this, 120)
        }
    }

    init {
        root.addView(keys, FrameLayout.LayoutParams(1, 1))
        root.addView(content, FrameLayout.LayoutParams(-1, -1))
        root.addView(progress, FrameLayout.LayoutParams(-2, -2, android.view.Gravity.CENTER))
        root.setOnClickListener { focusKeys() }
        content.setOnClickListener { focusKeys() }
        root.addOnLayoutChangeListener { _, left, top, right, bottom, _, _, _, _ -> resize(right - left, bottom - top) }
    }

    fun update(open: Boolean, view: JSONObject?) {
        if (open != shown) {
            shown = open
            root.visibility = if (open) View.VISIBLE else View.GONE
            if (open) { grid = 0 to 0; main.post(poll); focusKeys(); resize(root.width, root.height) }
            else {
                main.removeCallbacks(poll)
                activity.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(root.windowToken, 0)
                renderer.mount(content, null)
            }
        }
        if (!open) return
        progress.visibility = if (view == null) View.VISIBLE else View.GONE
        try { renderer.mount(content, view) } catch (_: Exception) { renderer.mount(content, null) }
    }

    fun stop() { shown = false; main.removeCallbacks(poll) }

    private fun focusKeys() {
        keys.requestFocus()
        activity.getSystemService(InputMethodManager::class.java)?.showSoftInput(keys, 0)
    }

    private fun resize(width: Int, height: Int) {
        if (!shown || width <= 0 || height <= 0) return
        val (cellWidth, cellHeight) = TerminalMetrics.cell(activity)
        // Space for the terminal's title and key rows around the grid.
        val chrome = activity.dp(132)
        val rows = ((height - chrome) / cellHeight).toInt().coerceIn(4, 200)
        val cols = ((width - activity.dp(16)) / cellWidth).toInt().coerceIn(20, 300)
        if (rows to cols == grid) return
        grid = rows to cols
        bridge.terminal(json("op" to "terminal_resize", "rows" to rows, "cols" to cols))
    }
}

/**
 * The terminal's keyboard target: typed text, Backspace, Enter, and hardware
 * keys with their modifiers go to Rust, which encodes them for the terminal.
 * Input is exact: no suggestions, correction, or capitals.
 */
class TerminalKeyView(context: Context, private val text: (String) -> Unit,
                      private val key: (String, Boolean, Boolean, Boolean) -> Unit) : View(context) {
    init {
        isFocusable = true; isFocusableInTouchMode = true
        contentDescription = "Terminal input"
        tag = "terminal-input"
    }

    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
            InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_NONE or EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
        return object : BaseInputConnection(this, true) {
            override fun setComposingText(value: CharSequence, newCursorPosition: Int): Boolean {
                // Native preedit never reaches the shell before commitment.
                if (value.length > 4096) return false
                return super.setComposingText(value, newCursorPosition)
            }
            override fun finishComposingText(): Boolean {
                val committed = editable?.toString().orEmpty()
                super.finishComposingText()
                editable?.clear()
                if (committed.isNotEmpty()) typed(committed)
                return true
            }
            override fun commitText(value: CharSequence, newCursorPosition: Int): Boolean {
                editable?.clear()
                typed(value.toString()); return true
            }
            override fun deleteSurroundingText(before: Int, after: Int): Boolean {
                if (!editable.isNullOrEmpty()) {
                    return super.deleteSurroundingText(before.coerceIn(0, 64), after.coerceIn(0, 64))
                }
                repeat(before.coerceIn(0, 64)) { key("backspace", false, false, false) }
                repeat(after.coerceIn(0, 64)) { key("delete", false, false, false) }
                return true
            }
            override fun deleteSurroundingTextInCodePoints(before: Int, after: Int): Boolean {
                if (!editable.isNullOrEmpty()) return super.deleteSurroundingTextInCodePoints(before, after)
                return deleteSurroundingText(before, after)
            }
            override fun performEditorAction(action: Int): Boolean { key("enter", false, false, false); return true }
            override fun sendKeyEvent(event: KeyEvent): Boolean {
                if (event.action == KeyEvent.ACTION_DOWN) handle(event)
                return true
            }
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean = handle(event) || super.onKeyDown(keyCode, event)

    private fun typed(value: String) {
        val parts = value.split('\n')
        parts.forEachIndexed { index, part ->
            if (part.isNotEmpty()) text(part)
            if (index < parts.size - 1) key("enter", false, false, false)
        }
    }

    private fun handle(event: KeyEvent): Boolean {
        val name = when (event.keyCode) {
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> "enter"
            KeyEvent.KEYCODE_DEL -> "backspace"
            KeyEvent.KEYCODE_FORWARD_DEL -> "delete"
            KeyEvent.KEYCODE_TAB -> if (event.isShiftPressed) "backtab" else "tab"
            KeyEvent.KEYCODE_ESCAPE -> "escape"
            KeyEvent.KEYCODE_DPAD_UP -> "up"
            KeyEvent.KEYCODE_DPAD_DOWN -> "down"
            KeyEvent.KEYCODE_DPAD_LEFT -> "left"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "right"
            KeyEvent.KEYCODE_MOVE_HOME -> "home"
            KeyEvent.KEYCODE_MOVE_END -> "end"
            KeyEvent.KEYCODE_PAGE_UP -> "page_up"
            KeyEvent.KEYCODE_PAGE_DOWN -> "page_down"
            KeyEvent.KEYCODE_INSERT -> "insert"
            in KeyEvent.KEYCODE_F1..KeyEvent.KEYCODE_F12 -> "f${event.keyCode - KeyEvent.KEYCODE_F1 + 1}"
            else -> null
        }
        if (name != null) { key(name, event.isCtrlPressed, event.isAltPressed, event.isShiftPressed); return true }
        val character = event.getUnicodeChar(event.metaState and (KeyEvent.META_CTRL_MASK or KeyEvent.META_ALT_MASK).inv())
        if (character == 0) return false
        val value = String(Character.toChars(character))
        if (event.isCtrlPressed || event.isAltPressed) key(value, event.isCtrlPressed, event.isAltPressed, event.isShiftPressed)
        else text(value)
        return true
    }
}
