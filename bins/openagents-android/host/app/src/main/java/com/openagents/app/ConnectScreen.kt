package com.openagents.app

import android.content.ClipboardManager
import android.content.Context
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import org.json.JSONObject

/**
 * Connect a computer (SCR-22) and Connected (SCR-23), over every tab. Rust
 * writes every word, decides what a scanned or pasted code is, and pairs;
 * this screen draws the camera and the paste field and hands Rust the text of
 * one code. Nothing here is kept.
 */
class ConnectScreen(
    private val activity: MainActivity,
    private val bridge: MobileBridge,
    private val scanner: QRScanner,
) {
    val root: FrameLayout = FrameLayout(activity).apply {
        setBackgroundColor(Palette.BACKGROUND)
        visibility = View.GONE
        isClickable = true
        tag = "connect-screen"
    }
    private var shown: String? = null
    private var pasting = false
    private var scanning = false

    val showing get() = root.visibility == View.VISIBLE

    /** Shows the screen Rust sent, or hides it when there is none. */
    fun update(screen: JSONObject?) {
        if (screen == null) {
            if (showing) { stopScan(); root.visibility = View.GONE; root.removeAllViews(); shown = null; pasting = false }
            return
        }
        root.visibility = View.VISIBLE
        val encoded = screen.toString()
        if (encoded == shown) return
        shown = encoded
        build(screen)
    }

    /** Back closes the screen, as its Close button does. */
    fun back() = bridge.connectClose()

    fun dispose() = stopScan()

    private fun build(screen: JSONObject) {
        stopScan()
        root.removeAllViews()
        val body = activity.column().apply { setPadding(activity.dp(16), activity.dp(16), activity.dp(16), activity.dp(24)) }
        val header = activity.row().apply { gravity = Gravity.CENTER_VERTICAL; minimumHeight = activity.dp(48) }
        val stage = screen.optString("stage")
        if (stage != "connected") {
            header.addView(activity.text("Close", 17f, Palette.LINK).apply {
                setPadding(0, activity.dp(10), activity.dp(16), activity.dp(10)); tag = "connect-close"
                setOnClickListener { bridge.connectClose() }
            })
        }
        header.addView(activity.text(screen.getString("title"), 17f).apply {
            typeface = Fonts.typeface(context, Fonts.BOLD); gravity = Gravity.CENTER
        }, LinearLayout.LayoutParams(0, -2, 1f))
        body.addView(header)
        when (stage) {
            "connected" -> connected(body, screen)
            "connecting" -> {
                val code = screen.textOrNull("code")
                if (code != null) {
                    body.add(activity.text(code, 44f).apply {
                        typeface = Fonts.typeface(context, Fonts.BOLD)
                        gravity = Gravity.CENTER; tag = "connect-code"
                    }, 24)
                } else {
                    body.add(ProgressBar(activity).apply { contentDescription = "Connecting" }, 24, -2)
                }
                screen.textOrNull("notice")?.let { body.add(activity.text(it, 17f).apply { tag = "connect-connecting" }, 12) }
            }
            else -> scan(body, screen)
        }
        root.addView(ScrollView(activity).apply { addView(body) }, FrameLayout.LayoutParams(-1, -1))
    }

    private fun scan(body: LinearLayout, screen: JSONObject) {
        val maxBytes = screen.optInt("max_bytes", 1024)
        if (!pasting) screen.optJSONObject("nearby")?.let { nearby(body, it) }
        if (!pasting) {
            val camera = activity.column()
            body.add(camera, 12)
            screen.textOrNull("prompt")?.let { body.add(activity.text(it, 17f).apply { tag = "connect-prompt" }, 8) }
            startScan(camera, maxBytes)
        }
        screen.textOrNull("notice")?.let { body.add(activity.text(it, 15f, Palette.SECONDARY).apply { tag = "connect-notice" }, 12) }
        screen.textOrNull("paste")?.let { paste ->
            if (pasting) {
                val field = EditText(activity).apply {
                    hint = paste; textSize = 15f
                    setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
                    background = activity.rounded(Palette.RAISED, 10f)
                    setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
                    inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                    importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
                    imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
                    isSaveEnabled = false; maxLines = 6; tag = "connect-paste-field"
                    setText(clipboard())
                }
                body.add(field, 12)
                val actions = activity.row().apply { gravity = Gravity.CENTER_VERTICAL }
                actions.addView(activity.pill("Connect", "connect-paste-send", primary = true) {
                    val code = field.text.toString(); field.setText("")
                    if (code.isNotBlank() && code.toByteArray().size <= maxBytes) bridge.connectCode(code)
                })
                actions.addView(View(activity), LinearLayout.LayoutParams(0, 1, 1f))
                actions.addView(activity.pill("Scan instead", "connect-scan") { pasting = false; shown = null; update(screen) })
                body.add(actions, 8)
            } else {
                body.add(activity.pill(paste, "connect-paste") { pasting = true; shown = null; update(screen) }, 16, -2)
            }
        }
        screen.textOrNull("get_app")?.let { body.add(activity.text(it, 13f, Palette.SECONDARY), 16) }
    }

    /**
     * Computers on this Wi-Fi. Anyone can name a computer anything, so a tap
     * only starts a pairing the computer approves after the codes match.
     */
    private fun nearby(body: LinearLayout, nearby: JSONObject) {
        body.add(activity.text(nearby.optString("title"), 17f).apply { typeface = Fonts.typeface(context, Fonts.BOLD) }, 12)
        val computers = nearby.optJSONArray("computers")
        for (index in 0 until (computers?.length() ?: 0)) {
            val row = computers!!.getJSONObject(index)
            val id = row.getString("id")
            body.add(activity.text(row.getString("label"), 17f).apply {
                background = activity.rounded(Palette.RAISED, 10f)
                setPadding(activity.dp(12), activity.dp(12), activity.dp(12), activity.dp(12))
                tag = "connect-nearby-${row.getString("label")}"
                setOnClickListener { bridge.connectNearby(id) }
            }, 8)
        }
        nearby.textOrNull("empty")?.let { body.add(activity.text(it, 13f, Palette.SECONDARY), 8) }
    }

    private fun connected(body: LinearLayout, screen: JSONObject) {
        body.gravity = Gravity.CENTER_HORIZONTAL
        body.add(activity.text("✓", 56f, Palette.SUCCESS).apply { gravity = Gravity.CENTER; importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO }, 32)
        screen.textOrNull("computer")?.let {
            body.add(activity.text(it, 22f).apply { typeface = Fonts.typeface(context, Fonts.BOLD); gravity = Gravity.CENTER; tag = "connect-computer" }, 12)
        }
        screen.textOrNull("notice")?.let { body.add(activity.text(it, 15f, Palette.SECONDARY).apply { gravity = Gravity.CENTER }, 12) }
        screen.textOrNull("done")?.let { body.add(activity.pill(it, "connect-done", primary = true) { bridge.connectClose() }, 24, -2) }
    }

    private fun startScan(camera: LinearLayout, maxBytes: Int) {
        activity.withCamera { granted ->
            if (!granted) {
                camera.addView(activity.text("Camera access is off. Turn it on in Settings, or paste the code instead.", 15f, Palette.SECONDARY))
                return@withCamera
            }
            if (!showing || pasting) return@withCamera
            scanning = true
            scanner.start(camera, maxBytes) { result ->
                scanning = false
                result.fold({ bridge.connectCode(it) }, { failure ->
                    camera.removeAllViews()
                    camera.addView(activity.text(failure.message ?: "The camera couldn't read this code.", 15f, Palette.SECONDARY))
                })
            }
        }
    }

    private fun stopScan() {
        if (scanning) { scanner.stop(); scanning = false }
    }

    private fun clipboard(): String {
        val manager = activity.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return ""
        return manager.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(activity)?.toString() ?: ""
    }
}
