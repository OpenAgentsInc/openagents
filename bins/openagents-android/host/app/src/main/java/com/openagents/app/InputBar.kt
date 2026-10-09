package com.openagents.app

import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject

/**
 * The keyboard or camera for a value Rust asks for (an input request on the
 * Computers surface). Rust validates every value; nothing here is
 * kept. A secret request uses a masked field with no suggestions or autofill,
 * and the field is cleared after each send.
 */
class InputBar(
    private val activity: MainActivity,
    private val scanner: QRScanner,
) {
    val root: LinearLayout = activity.column().apply {
        background = activity.rounded(Palette.SURFACE, 16f, Palette.BORDER)
        setPadding(activity.dp(12), activity.dp(12), activity.dp(12), activity.dp(12))
        tag = "input-bar"
    }
    private var token: String? = null
    private var input: JSONObject? = null
    private var submit: (String) -> Unit = {}
    private var cancel: () -> Unit = {}
    private val error = activity.text("", 13f, Palette.SECONDARY)
    private val camera = activity.column()
    private val field = EditText(activity)
    private val actions = activity.row()
    private val send = button("Submit") { send(field.text.toString()) }
    private val scan = button("Scan QR code") { startScan() }
    private val type = button("Type instead") { stopScan() }
    private var scanning = false

    init {
        field.addTextChangedListener(object : android.text.TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
            override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) = Unit
            override fun afterTextChanged(s: android.text.Editable?) {
                send.isEnabled = !s.isNullOrBlank(); send.alpha = if (send.isEnabled) 1f else 0.4f
            }
        })
    }

    fun show(next: JSONObject?, busy: Boolean, submit: (String) -> Unit, cancel: () -> Unit) {
        this.submit = submit; this.cancel = cancel
        root.visibility = if (next == null) View.GONE else View.VISIBLE
        if (next == null) { if (token != null) { stopScan(); token = null; input = null }; return }
        val nextToken = next.getString("token")
        if (nextToken != token) build(next)
        send.isEnabled = !busy && field.text.isNotBlank()
        send.alpha = if (send.isEnabled) 1f else 0.4f
    }

    fun dispose() = stopScan()

    private fun build(next: JSONObject) {
        stopScan()
        token = next.getString("token"); input = next
        root.removeAllViews()
        error.text = ""; error.visibility = View.GONE
        root.addView(activity.text(next.getString("label"), 17f).apply { typeface = Fonts.typeface(context, Fonts.BOLD) })
        root.addView(activity.text(next.getString("prompt"), 13f, Palette.SECONDARY), LinearLayout.LayoutParams(-1, -2).apply {
            topMargin = activity.dp(4) })
        root.addView(error)
        root.addView(camera)
        val secret = next.optBoolean("secret")
        field.apply {
            setText("")
            hint = next.getString("label")
            textSize = 15f
            setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
            background = activity.rounded(Palette.RAISED, 10f)
            setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
            isSaveEnabled = false
            tag = "computers-input"
            if (secret) {
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
                imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
                maxLines = 1
            } else {
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
                imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
                maxLines = 6
            }
        }
        (field.parent as? LinearLayout)?.removeView(field)
        root.addView(field, LinearLayout.LayoutParams(-1, -2).apply { topMargin = activity.dp(8) })
        actions.removeAllViews()
        actions.gravity = Gravity.CENTER_VERTICAL
        actions.addView(send)
        if (next.optBoolean("scan")) actions.addView(scan, LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
        actions.addView(type, LinearLayout.LayoutParams(-2, -2).apply { marginStart = activity.dp(8) })
        actions.addView(View(activity), LinearLayout.LayoutParams(0, 1, 1f))
        actions.addView(button("Cancel") { field.setText(""); stopScan(); cancel() })
        (actions.parent as? LinearLayout)?.removeView(actions)
        root.addView(actions, LinearLayout.LayoutParams(-1, -2).apply { topMargin = activity.dp(8) })
        type.visibility = View.GONE
        if (next.optBoolean("scan")) startScan()
    }

    private fun startScan() {
        val input = input ?: return
        activity.withCamera { granted ->
            if (!granted) { fail("Camera access is off. Turn it on in Settings, or type the code instead."); return@withCamera }
            if (input !== this.input) return@withCamera
            scanning = true
            field.visibility = View.GONE; scan.visibility = View.GONE; send.visibility = View.GONE
            type.visibility = View.VISIBLE
            camera.removeAllViews()
            scanner.start(camera, input.getInt("max_bytes")) { result ->
                result.fold({ send(it) }, { fail(it.message ?: "The camera couldn't read this code."); stopScan() })
            }
        }
    }

    private fun stopScan() {
        if (scanning) scanner.stop()
        scanning = false
        camera.removeAllViews()
        field.visibility = View.VISIBLE; send.visibility = View.VISIBLE
        scan.visibility = View.VISIBLE; type.visibility = View.GONE
    }

    private fun fail(message: String) { error.text = message; error.visibility = View.VISIBLE }

    private fun send(value: String) {
        val input = input ?: return
        if (value.isBlank()) return
        if (value.toByteArray().size > input.getInt("max_bytes")) { fail("That's too long. Copy it again."); return }
        error.visibility = View.GONE
        field.setText("")
        stopScan()
        submit(value)
    }

    private fun button(label: String, action: () -> Unit): Button = Button(activity).apply {
        text = label; isAllCaps = false; textSize = 14f; setTextColor(Palette.PRIMARY)
        background = activity.rounded(Palette.RAISED, 10f)
        stateListAnimator = null
        minHeight = activity.dp(40); minimumHeight = activity.dp(40)
        setPadding(activity.dp(12), 0, activity.dp(12), 0)
        setOnClickListener { action() }
    }
}

/** Draws an invitation QR code Rust rendered: one string of `1` (dark) and `0` per row. */
internal fun qrBitmap(qr: JSONObject): android.graphics.Bitmap? {
    val size = qr.optInt("size")
    val rows = qr.optJSONArray("rows") ?: return null
    if (size !in 1..200 || rows.length() != size) return null
    val scale = 8
    val bitmap = android.graphics.Bitmap.createBitmap(size * scale, size * scale, android.graphics.Bitmap.Config.ARGB_8888)
    for (y in 0 until size) {
        val row = rows.getString(y)
        if (row.length != size) return null
        for (x in 0 until size) {
            val color = if (row[x] == '1') android.graphics.Color.BLACK else android.graphics.Color.WHITE
            for (dy in 0 until scale) for (dx in 0 until scale) bitmap.setPixel(x * scale + dx, y * scale + dy, color)
        }
    }
    return bitmap
}

internal fun TextView.bold(): TextView { typeface = Fonts.typeface(context, Fonts.BOLD); return this }
