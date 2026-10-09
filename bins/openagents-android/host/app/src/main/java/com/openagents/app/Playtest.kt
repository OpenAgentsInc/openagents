// Report a problem, My reports, and Account > Playtest. Rust fills in and
// checks every report, decides whether a screenshot may be offered, seals
// the report to the triage key, and keeps the playtest log; these views only
// collect what the tester writes and chooses, and show the exact screenshot
// and log before anything is sent. They follow the iOS `Playtest.swift`.
package com.openagents.app

import android.app.AlertDialog
import android.app.Dialog
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.util.Base64
import android.view.Gravity
import android.view.PixelCopy
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.SeekBar
import android.widget.Switch
import org.json.JSONObject
import java.io.ByteArrayOutputStream

/** The device facts a report carries, in the forms Rust accepts. */
internal object ReportDevice {
    val version: String get() = BuildConfig.VERSION_NAME
    val build: String get() = BuildConfig.VERSION_CODE.toString()
    /** The Android release, or the API level when the release isn't numeric. */
    val os: String get() = Build.VERSION.RELEASE.takeIf { r ->
        r.isNotEmpty() && r.length <= 16 && r.split('.').size <= 4 && r.split('.').all { p -> p.isNotEmpty() && p.all(Char::isDigit) }
    } ?: Build.VERSION.SDK_INT.toString()
    /** Manufacturer and model, limited to the characters and length Rust accepts. */
    val model: String get() = "${Build.MANUFACTURER} ${Build.MODEL}"
        .filter { it.isLetterOrDigit() && it.code < 128 || it in " ,._-()" }.trim().take(40).ifEmpty { "Android" }
}

/** The screenshot, cropped at the top and bottom, as the JPEG that would be sent. */
internal object ReportImage {
    fun cropped(image: Bitmap, top: Double, bottom: Double): Bitmap? {
        val from = Math.round(image.height * top).toInt()
        val to = Math.round(image.height * (1 - bottom)).toInt()
        if (to - from < 16) return null
        return Bitmap.createBitmap(image, 0, from, image.width, to - from)
    }

    /** Nil when it can't be made small enough to send. */
    fun jpeg(image: Bitmap): Triple<ByteArray, Int, Int>? {
        for ((width, quality) in listOf(360 to 55, 300 to 40, 240 to 30)) {
            val scale = minOf(1.0, width.toDouble() / image.width)
            val w = Math.round(image.width * scale).toInt().coerceAtLeast(1)
            val h = Math.round(image.height * scale).toInt().coerceAtLeast(1)
            val small = Bitmap.createScaledBitmap(image, w, h, true)
            val out = ByteArrayOutputStream()
            small.compress(Bitmap.CompressFormat.JPEG, quality, out)
            if (out.size() <= 24_000) return Triple(out.toByteArray(), w, h)
        }
        return null
    }
}

internal class Playtest(private val activity: MainActivity, private val bridge: MobileBridge) {
    private val main = Handler(Looper.getMainLooper())
    private fun dialog() = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)

    /** My reports and the playtest log, the latest Rust answer. */
    var reports: JSONObject? = null; private set
    /** The trainer packet, whose `playtest` object is the playtest card. */
    private var card: JSONObject? = null
    private var logOpen = false

    fun loadReports(refresh: () -> Unit) = bridge.reports { reports = it; refresh() }

    fun loadCard(refresh: () -> Unit) = bridge.trainer(preview = activity.xpPreview) { next ->
        if (next.toString() != card?.toString()) { card = next; refresh() }
    }

    val sending get() = reports?.optJSONArray("reports")?.objects()?.any { it.optString("status") == "sending" } == true

    // Give feedback (#10127)

    /** Give feedback on `text`, selected in the transcript row `row`. */
    fun feedback(text: String, row: String, tab: String, route: String) = FeedbackSheet(text, row, tab, route).show()

    /** The selected text, quoted, and a comment; Send files it as a report. */
    private inner class FeedbackSheet(val text: String, val row: String, val tab: String, val route: String) {
        private val dialog = Dialog(activity, android.R.style.Theme_Black_NoTitleBar_Fullscreen)
        private val content = FrameLayout(activity).apply { setBackgroundColor(Palette.BACKGROUND) }
        private val comment = EditText(activity).apply {
            hint = "What's wrong or what should change?"; setHintTextColor(Palette.TERTIARY); setTextColor(Palette.PRIMARY)
            textSize = 16f; minLines = 4; gravity = Gravity.TOP or Gravity.START; tag = "feedback-comment"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            background = activity.rounded(Palette.SURFACE, 10f)
            setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
        }
        private var sending = false
        private var said: String? = null
        private var error: String? = null

        fun show() {
            dialog.setContentView(content)
            dialog.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
            paint()
            dialog.show()
        }

        private fun paint() {
            (comment.parent as? ViewGroup)?.removeView(comment)
            val screen = activity.column()
            screen.addView(activity.row().apply {
                gravity = Gravity.CENTER_VERTICAL; minimumHeight = activity.dp(52)
                setPadding(activity.dp(8), 0, activity.dp(8), 0)
                addView(activity.label(if (said == null) "Cancel" else "Close", 17f).apply {
                    setPadding(activity.dp(8), activity.dp(10), activity.dp(8), activity.dp(10)); setOnClickListener { dialog.dismiss() }
                }, LinearLayout.LayoutParams(activity.dp(96), -2))
                addView(activity.label("Give feedback", 17f, bold = true).apply { gravity = Gravity.CENTER }, LinearLayout.LayoutParams(0, -2, 1f))
                addView(activity.label(if (said == null) "Send" else "", 17f, Palette.LINK, key = "feedback-send", bold = true).apply {
                    gravity = Gravity.END; setPadding(activity.dp(8), activity.dp(10), activity.dp(8), activity.dp(10))
                    setOnClickListener { send() }
                }, LinearLayout.LayoutParams(activity.dp(96), -2))
            })
            val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(32)) }
            body.add(activity.label(text.take(600), 15f, Palette.SECONDARY, key = "feedback-quote").apply {
                background = activity.rounded(Palette.SURFACE, 10f)
                setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
            }, 8)
            comment.isEnabled = said == null
            body.add(comment, 16)
            said?.let { body.add(activity.label(it, 15f, key = "feedback-sent"), 16) }
            error?.let { body.add(activity.label(it, 14f, Palette.FAILURE, key = "feedback-error"), 16) }
            screen.addView(ScrollView(activity).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
            content.removeAllViews()
            content.addView(screen, FrameLayout.LayoutParams(-1, -1))
        }

        private fun send() {
            if (sending || said != null || comment.text.isBlank()) return
            val form = json("app_version" to ReportDevice.version, "build" to ReportDevice.build,
                "device" to ReportDevice.model, "os_version" to ReportDevice.os,
                "tab" to tab, "route" to route, "text" to text, "comment" to comment.text.toString(), "row" to row)
            sending = true; error = null
            bridge.sendFeedback(form) { packet ->
                sending = false
                reports = packet
                said = packet.textOrNull("feedback")
                if (said == null) error = packet.textOrNull("error") ?: "The feedback couldn't be sent."
                paint()
            }
        }
    }

    // Report a problem

    /**
     * Opens Report a problem for the screen on view. Rust says whether it
     * may be captured; the Wallet and key screens never are.
     */
    fun start(tab: String, route: String, world: SurfaceView?) {
        bridge.reportDraft(tab, route) { draft ->
            if (draft.optBoolean("screenshot_allowed")) capture(world) { ReportSheet(draft, it).show() }
            else ReportSheet(draft, null).show()
        }
    }

    /** The window as it looks now, with the Verse world under it when it shows. */
    private fun capture(world: SurfaceView?, done: (Bitmap?) -> Unit) {
        val decor = activity.window.decorView
        if (decor.width == 0 || decor.height == 0) { done(null); return }
        val window = Bitmap.createBitmap(decor.width, decor.height, Bitmap.Config.ARGB_8888)
        PixelCopy.request(activity.window, window, { result ->
            if (result != PixelCopy.SUCCESS) { done(null); return@request }
            val surface = world?.takeIf { it.isShown && it.width > 0 && it.height > 0 && it.holder.surface.isValid }
            if (surface == null) { done(window); return@request }
            val under = Bitmap.createBitmap(surface.width, surface.height, Bitmap.Config.ARGB_8888)
            PixelCopy.request(surface, under, { worldResult ->
                if (worldResult != PixelCopy.SUCCESS) { done(window); return@request }
                val at = IntArray(2).also { surface.getLocationInWindow(it) }
                val composed = Bitmap.createBitmap(decor.width, decor.height, Bitmap.Config.ARGB_8888)
                Canvas(composed).apply {
                    drawBitmap(under, null, Rect(at[0], at[1], at[0] + surface.width, at[1] + surface.height), null)
                    drawBitmap(window, 0f, 0f, null)
                }
                done(composed)
            }, main)
        }, main)
    }

    /** Report a problem: what happened, what you expected, the steps, and a kind. */
    private inner class ReportSheet(private var draft: JSONObject, private val image: Bitmap?) {
        private val dialog = Dialog(activity, android.R.style.Theme_Black_NoTitleBar_Fullscreen)
        private var kind = "bug"
        private val happened: EditText = field("What happened", 3, "report-happened")
        private val expected: EditText = field("What you expected", 2, "report-expected")
        private val steps: EditText = field("Steps", 2, "report-steps")
        private var quote = false
        private var includeTask = false
        private var includeShot = false
        private var includeLog = draft.optBoolean("logging")
        private var includeChat = false
        private var cropTop = 0.0
        private var cropBottom = 0.0
        private var sending = false
        private var error: String? = null
        private val content = FrameLayout(activity).apply { setBackgroundColor(Palette.BACKGROUND) }

        fun show() {
            dialog.setContentView(content)
            dialog.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
            form()
            dialog.show()
        }

        private fun field(hint: String, lines: Int, key: String): EditText = EditText(activity).apply {
            this.hint = hint; setHintTextColor(Palette.TERTIARY); setTextColor(Palette.PRIMARY); textSize = 16f
            minLines = lines; gravity = Gravity.TOP or Gravity.START; tag = key
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            background = activity.rounded(Palette.SURFACE, 10f)
            setPadding(activity.dp(12), activity.dp(10), activity.dp(12), activity.dp(10))
            addTextChangedListener(object : android.text.TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun afterTextChanged(s: android.text.Editable?) { sendButton?.enabled(canSend()) }
            })
        }

        private var sendButton: View? = null
        private fun canSend(): Boolean = !sending && happened.text.toString().isNotBlank()

        private fun header(title: String, left: Pair<String, () -> Unit>?, right: Pair<String, () -> Unit>?, rightKey: String): View =
            activity.row().apply {
                gravity = Gravity.CENTER_VERTICAL; minimumHeight = activity.dp(52)
                setPadding(activity.dp(8), 0, activity.dp(8), 0)
                addView(activity.label(left?.first ?: "", 17f).apply {
                    setPadding(activity.dp(8), activity.dp(10), activity.dp(8), activity.dp(10)); left?.let { l -> setOnClickListener { l.second() } }
                }, LinearLayout.LayoutParams(activity.dp(96), -2))
                addView(activity.label(title, 17f, bold = true).apply { gravity = Gravity.CENTER }, LinearLayout.LayoutParams(0, -2, 1f))
                addView(activity.label(right?.first ?: "", 17f, Palette.LINK, key = rightKey, bold = true).apply {
                    gravity = Gravity.END; setPadding(activity.dp(8), activity.dp(10), activity.dp(8), activity.dp(10))
                    right?.let { r -> setOnClickListener { if (isEnabled) r.second() } }
                    if (right != null) sendButton = this
                }, LinearLayout.LayoutParams(activity.dp(96), -2))
            }

        private fun toggle(title: String, on: Boolean, key: String, changed: (Boolean) -> Unit): View = activity.row().apply {
            gravity = Gravity.CENTER_VERTICAL; minimumHeight = activity.dp(48)
            addView(activity.label(title, 16f), LinearLayout.LayoutParams(0, -2, 1f))
            addView(Switch(activity).apply { isChecked = on; tag = key; contentDescription = title
                setOnCheckedChangeListener { _, value -> changed(value) } })
        }

        private fun LinearLayout.card(footer: String? = null, rows: LinearLayout.() -> Unit) {
            add(activity.column().apply {
                background = activity.rounded(Palette.SURFACE, 12f)
                setPadding(activity.dp(16), activity.dp(8), activity.dp(16), activity.dp(8))
                rows()
            }, 16)
            footer?.let { add(activity.label(it, 13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 6) }
        }

        private fun heading(text: String) = activity.label(text.uppercase(), 13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, 0, 0) }

        private fun detach(vararg views: View) = views.forEach { (it.parent as? ViewGroup)?.removeView(it) }

        private fun shot(): Bitmap? = if (includeShot && image != null) ReportImage.cropped(image, cropTop, cropBottom) else null

        private fun form() {
            detach(happened, expected, steps)
            val ready = draft.optBoolean("triage_ready")
            val screen = activity.column()
            screen.addView(header("Report a problem", "Cancel" to { dialog.dismiss() }, (if (ready) "Send" else "Save") to { send() }, "report-send"))
            sendButton?.enabled(canSend())
            val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(32)) }
            val kinds = draft.optJSONArray("kinds")?.objects() ?: emptyList()
            val hint = activity.label("", 13f, Palette.SECONDARY)
            val choices = FlowLayout(activity, activity.dp(6)).apply { tag = "report-kind" }
            fun paintKinds() {
                for (i in 0 until choices.childCount) {
                    val chip = choices.getChildAt(i) as android.widget.TextView
                    val selected = chip.tag == "report-kind-$kind"
                    chip.background = if (selected) activity.rounded(Palette.PRIMARY, 16f) else activity.rounded(Palette.RAISED, 16f, Palette.BORDER)
                    chip.setTextColor(if (selected) Palette.BACKGROUND else Palette.PRIMARY)
                    chip.isSelected = selected
                }
                hint.text = kinds.firstOrNull { it.optString("value") == kind }?.optString("hint") ?: ""
            }
            for (choice in kinds) choices.addView(activity.text(choice.getString("label"), 15f).apply {
                tag = "report-kind-${choice.getString("value")}"
                setPadding(activity.dp(14), activity.dp(8), activity.dp(14), activity.dp(8))
                setOnClickListener { kind = choice.getString("value"); paintKinds() }
            })
            paintKinds()
            body.add(choices, 8); body.add(hint, 6)
            body.add(heading("What happened"), 16); body.add(happened, 6)
            body.add(heading("What you expected"), 16); body.add(expected, 6)
            body.add(heading("Steps"), 16); body.add(steps, 6)
            draft.textOrNull("task")?.let { task ->
                body.card(task) { add(toggle("Attach this chat's task ID", includeTask, "report-task") { includeTask = it }) }
            }
            screenshotSection(body)
            val chatLines = draft.optJSONArray("chat_lines").strings()
            if (chatLines.isNotEmpty()) {
                body.card("Off by default. Helps us improve our answers: exactly these messages, sent only to the triage team.") {
                    add(toggle("Share this chat (${chatLines.size} messages)", includeChat, "report-share-chat") { includeChat = it })
                    addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
                    add(activity.label(chatLines.joinToString("\n"), 12f, Palette.SECONDARY, key = "report-chat-lines")
                        .apply { setPadding(0, activity.dp(8), 0, activity.dp(8)) })
                }
            }
            if (draft.optBoolean("logging")) {
                val lines = draft.optJSONArray("log_lines").strings()
                body.card("Tab, screen, event, and time only: exactly these lines.") {
                    add(toggle("Attach the playtest log (${lines.size} events)", includeLog, "report-log") { includeLog = it })
                    addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
                    add(activity.label(lines.joinToString("\n").ifEmpty { "No events yet." }, 12f, Palette.SECONDARY, mono = true,
                        key = "report-log-lines").apply { setPadding(0, activity.dp(8), 0, activity.dp(8)) })
                }
            }
            body.card(draft.optString("privacy")) {
                add(toggle("You may quote my words in a public issue", quote, "report-quote") { quote = it })
            }
            body.add(heading("Sent with the report"), 16)
            body.add(activity.label("${ReportDevice.version} (${ReportDevice.build}) · ${draft.optString("tab")}/${draft.optString("route")} · " +
                "${ReportDevice.model} · Android ${ReportDevice.os}", 12f, Palette.SECONDARY, mono = true, key = "report-context")
                .apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 6)
            if (!ready) body.card {
                add(activity.label("This build can't send reports yet. Yours is saved on this phone and is sent by a later build. " +
                    "To report now, use the GitHub form.", 14f).apply { setPadding(0, activity.dp(6), 0, activity.dp(6)) })
                add(activity.label("Open the Playtest report form ↗", 16f, Palette.LINK, key = "report-fallback").apply {
                    setPadding(0, activity.dp(10), 0, activity.dp(10))
                    setOnClickListener { activity.openLink(draft.optString("fallback")) }
                })
            }
            error?.let { body.add(activity.label(it, 14f, Palette.FAILURE, key = "report-error"), 16) }
            screen.addView(ScrollView(activity).apply { addView(body) }, LinearLayout.LayoutParams(-1, 0, 1f))
            content.removeAllViews()
            content.addView(screen, FrameLayout.LayoutParams(-1, -1))
        }

        private fun screenshotSection(body: LinearLayout) {
            if (!draft.optBoolean("screenshot_allowed")) {
                body.card { add(activity.label("No screenshot from the Wallet or a key screen. Describe it in words.", 14f, Palette.SECONDARY,
                    key = "report-no-screenshot").apply { setPadding(0, activity.dp(8), 0, activity.dp(8)) }) }
                return
            }
            image ?: return
            body.card("Off by default. The picture shown is exactly what's sent, made smaller.") {
                val preview = ImageView(activity).apply { adjustViewBounds = true; tag = "report-screenshot-preview"
                    contentDescription = "The screenshot that would be sent" }
                val controls = activity.column()
                fun repaint() {
                    preview.setImageBitmap(shot())
                    controls.visibility = if (includeShot) View.VISIBLE else View.GONE
                }
                add(toggle("Attach a screenshot", includeShot, "report-screenshot") { includeShot = it; repaint() })
                controls.add(preview.also { it.maxHeight = activity.dp(260) }, 4)
                for ((title, get, set) in listOf<Triple<String, () -> Double, (Double) -> Unit>>(
                    Triple("Crop top", { cropTop }, { v -> cropTop = v }), Triple("Crop bottom", { cropBottom }, { v -> cropBottom = v }))) {
                    controls.add(activity.label(title, 14f, Palette.SECONDARY), 8)
                    controls.add(SeekBar(activity).apply {
                        max = 45; progress = (get() * 100).toInt(); contentDescription = title
                        setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                            override fun onProgressChanged(bar: SeekBar?, value: Int, user: Boolean) { set(value / 100.0); preview.setImageBitmap(shot()) }
                            override fun onStartTrackingTouch(bar: SeekBar?) {}
                            override fun onStopTrackingTouch(bar: SeekBar?) {}
                        })
                    })
                }
                add(controls)
                repaint()
            }
        }

        private fun send() {
            if (!canSend()) return
            val form = json("app_version" to ReportDevice.version, "build" to ReportDevice.build,
                "device" to ReportDevice.model, "os_version" to ReportDevice.os,
                "tab" to draft.optString("tab"), "route" to draft.optString("route"), "kind" to kind,
                "happened" to happened.text.toString(), "expected" to expected.text.toString(), "steps" to steps.text.toString(),
                "quote" to quote, "include_task" to includeTask, "include_log" to (includeLog && draft.optBoolean("logging")),
                "log_digest" to draft.optString("log_digest"),
                "include_chat" to (includeChat && draft.optJSONArray("chat_lines").strings().isNotEmpty()),
                "chat_digest" to draft.optString("chat_digest"))
            if (draft.optBoolean("screenshot_allowed")) shot()?.let(ReportImage::jpeg)?.let { (bytes, w, h) ->
                form.put("screenshot", json("jpeg_base64" to Base64.encodeToString(bytes, Base64.NO_WRAP), "width" to w, "height" to h))
            }
            sending = true; error = null; sendButton?.enabled(false)
            bridge.sendReport(form) { packet ->
                sending = false
                reports = packet
                val row = packet.objectOrNull("sent")
                if (row != null) { receipt(row); return@sendReport }
                error = packet.textOrNull("error") ?: "The report couldn't be filed."
                // The log or the chat moved on: show the current one before sending.
                if (error?.contains("log changed") == true || error?.contains("chat changed") == true) {
                    bridge.reportDraft(draft.optString("tab"), draft.optString("route")) { draft = it; form() }
                } else form()
            }
        }

        private fun receipt(row: JSONObject) {
            val waiting = row.optString("status") == "waiting"
            val screen = activity.column()
            screen.addView(header("Report a problem", null, "Done" to { dialog.dismiss() }, "report-done"))
            screen.addView(activity.column().apply {
                setPadding(activity.dp(24), activity.dp(16), activity.dp(24), activity.dp(16))
                add(activity.label(if (waiting) "Saved on this phone" else "Report filed", 24f, bold = true, key = "report-receipt"))
                row.textOrNull("code")?.let { add(activity.label(it, 20f, mono = true, selectable = true, key = "report-code"), 8) }
                add(activity.label(if (waiting) "It's sent by a later build. You'll find it in Account, My reports."
                    else "Quote this code if you talk to us about it. You'll find it in Account, My reports.", 15f, Palette.SECONDARY), 8)
            })
            content.removeAllViews()
            content.addView(screen, FrameLayout.LayoutParams(-1, -1))
        }
    }

    // My reports

    /** The reports this phone filed, newest first. */
    fun myReports(report: () -> Unit): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        val packet = reports
        val rows = packet?.optJSONArray("reports")?.objects() ?: emptyList()
        body.add(activity.column().apply {
            background = activity.rounded(Palette.SURFACE, 12f)
            setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(4))
            tag = "reports-list"
            if (packet == null) add(activity.label("Reading your reports…", 15f, Palette.SECONDARY).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) })
            else if (rows.isEmpty()) add(activity.label("No reports yet. Long-press the tab bar on any screen, or use Report a problem in Account.",
                15f, Palette.SECONDARY).apply { setPadding(0, activity.dp(12), 0, activity.dp(12)) })
            rows.forEachIndexed { index, row ->
                if (index > 0) addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
                add(activity.column().apply {
                    setPadding(0, activity.dp(10), 0, activity.dp(10))
                    tag = "report-row-${row.optString("id")}"
                    val top = activity.row()
                    top.addView(activity.label(row.textOrNull("code") ?: "No code yet", 16f, mono = true, bold = true), LinearLayout.LayoutParams(0, -2, 1f))
                    top.addView(activity.label(row.optString("status_label"), 12f,
                        if (row.optString("status") == "sent") Palette.SECONDARY else 0xFFFFD60A.toInt()))
                    add(top)
                    add(activity.label(row.optString("summary"), 14f).apply { maxLines = 2 }, 2)
                    add(activity.label("${row.optString("kind_label")} · ${row.optString("place")} · ${row.optString("build")}" +
                        (if (row.optBoolean("screenshot")) " · screenshot" else "") + (if (row.optBoolean("log")) " · playtest log" else "") +
                        (if (row.optBoolean("published")) " · public record" else ""),
                        12f, Palette.SECONDARY), 2)
                    row.textOrNull("error")?.let { add(activity.label(it, 12f, Palette.FAILURE), 2) }
                })
            }
        }, 8)
        body.add(activity.pill("Report a problem", "reports-report", primary = true, action = report), 16)
        body.add(activity.label(if (packet != null && !packet.optBoolean("triage_ready"))
            "This build can't send reports yet; they wait here and are sent by a later build."
            else "Reports go privately to the OpenAgents triage team. Accepted ones become public GitHub issues that we write.",
            13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 8)
        return ScrollView(activity).apply { addView(body) }
    }

    // Account > Playtest

    /** The playtest card, playtest logging (set by the build, no switch), and the way to report. */
    fun screen(refresh: () -> Unit, report: () -> Unit, openReports: () -> Unit): View {
        val body = activity.column().apply { setPadding(activity.dp(16), 0, activity.dp(16), activity.dp(24)) }
        playtestCard(body)
        val log = reports?.objectOrNull("log")
        val events = log?.optInt("events") ?: 0
        if (log != null) {
            body.add(activity.label("PLAYTEST LOGGING", 13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, 0, 0) }, 20)
            body.add(activity.column().apply {
                background = activity.rounded(Palette.SURFACE, 12f)
                setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(4))
                add(activity.label(log.optString("note"), 16f, key = "playtest-logging").apply {
                    setPadding(0, activity.dp(12), 0, activity.dp(12))
                })
                if (events > 0) {
                    addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
                    add(activity.label((if (events == 1) "1 event" else "$events events") + if (logOpen) "  ▾" else "  ›", 16f, key = "playtest-log-toggle").apply {
                        setPadding(0, activity.dp(12), 0, activity.dp(12)); setOnClickListener { logOpen = !logOpen; refresh() }
                    })
                    if (logOpen) add(activity.label(log.optJSONArray("lines").strings().joinToString("\n"), 12f, Palette.SECONDARY,
                        mono = true, key = "playtest-log-lines").apply { setPadding(0, 0, 0, activity.dp(8)) })
                    addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
                    add(activity.label("Delete the log", 16f, Palette.FAILURE, key = "playtest-log-delete").apply {
                        setPadding(0, activity.dp(12), 0, activity.dp(12))
                        setOnClickListener {
                            dialog().setTitle("Delete the playtest log?")
                                .setPositiveButton("Delete") { _, _ -> bridge.playtestClear { reports = it; refresh() } }
                                .setNegativeButton("Cancel", null).show()
                        }
                    })
                }
            }, 6)
            if (log.optBoolean("on")) body.add(activity.label("This phone notes which tab and screen you're on, error codes, and when. Never messages, prompts, keys, " +
                "recovery words, invoices, addresses, or amounts. The log stays on this phone and goes only in a report you preview.",
                13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 6)
        }
        body.add(activity.column().apply {
            background = activity.rounded(Palette.SURFACE, 12f)
            setPadding(activity.dp(16), activity.dp(4), activity.dp(16), activity.dp(4))
            add(activity.label("Report a problem", 16f, Palette.LINK, key = "playtest-report").apply {
                setPadding(0, activity.dp(12), 0, activity.dp(12)); setOnClickListener { report() }
            })
            addView(activity.divider(), LinearLayout.LayoutParams(-1, 1))
            add(activity.row().apply {
                gravity = Gravity.CENTER_VERTICAL; tag = "playtest-reports"
                setPadding(0, activity.dp(12), 0, activity.dp(12))
                addView(activity.label("My reports", 16f), LinearLayout.LayoutParams(0, -2, 1f))
                addView(activity.text("›", 22f, Palette.TERTIARY))
                setOnClickListener { openReports() }
            })
        }, 20)
        body.add(activity.label("Tip: long-press the tab bar on any screen to report what's on it.", 13f, Palette.SECONDARY)
            .apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 6)
        return ScrollView(activity).apply { addView(body) }
    }

    /**
     * Sessions, accepted reports, fixes verified, playtest XP beside (never
     * inside) the trainer level, and titles, which Rust reads from the
     * playtest referee.
     */
    private fun playtestCard(body: LinearLayout) {
        val trainer = card
        val playtest = trainer?.objectOrNull("playtest")
        // The card shows only once the playtest referee's awards are read:
        // while its key is unpublished there are no real numbers to show.
        val state = playtest?.optString("state")
        if (state != "ready" && state != "preview") return
        body.add(activity.label("PLAYTEST CARD", 13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, 0, 0) }, 8)
        body.add(activity.column().apply {
            background = activity.rounded(Palette.SURFACE, 12f)
            setPadding(activity.dp(16), activity.dp(12), activity.dp(16), activity.dp(12))
            tag = "playtest-card"
            if (playtest?.optString("state") == "preview") add(activity.label("Preview: a labeled fixture, not real awards.", 13f, 0xFFFFD60A.toInt()))
            add(activity.row().apply {
                gravity = Gravity.BOTTOM
                addView(activity.label("${playtest?.optLong("xp") ?: 0} playtest XP", 24f, bold = true, key = "playtest-xp"), LinearLayout.LayoutParams(0, -2, 1f))
                if (trainer != null) addView(activity.label("Trainer: ${trainer.optLong("xp")} XP · lv ${trainer.optInt("level")}", 13f, Palette.SECONDARY))
            }, 4)
            add(activity.row().apply {
                for ((key, label) in listOf("sessions" to "sessions", "accepted_reports" to "accepted reports", "fixes_verified" to "fixes verified")) {
                    addView(activity.column().apply {
                        add(activity.label("${playtest?.optInt(key) ?: 0}", 20f, key = "playtest-$key"))
                        add(activity.label(label, 12f, Palette.SECONDARY))
                    }, LinearLayout.LayoutParams(0, -2, 1f))
                }
            }, 10)
            val titles = playtest?.optJSONArray("titles").strings()
            if (titles.isNotEmpty()) add(activity.label(titles.joinToString(" · ") { it.uppercase() }, 13f, mono = true, bold = true), 10)
            add(activity.label(status(playtest), 13f, Palette.SECONDARY), 10)
            for (award in playtest?.optJSONArray("awards")?.objects() ?: emptyList()) {
                add(activity.row().apply {
                    setPadding(0, activity.dp(8), 0, activity.dp(8))
                    addView(activity.label(award.optString("title"), 14f), LinearLayout.LayoutParams(0, -2, 1f))
                    addView(activity.label("+${award.optLong("xp")} XP", 14f))
                    val link = award.optString("link")
                    if (link.startsWith("https://")) setOnClickListener { activity.openLink(link) }
                })
            }
        }, 6)
        playtest?.textOrNull("note")?.let { body.add(activity.label(it, 13f, Palette.SECONDARY).apply { setPadding(activity.dp(16), 0, activity.dp(16), 0) }, 6) }
    }

    private fun status(playtest: JSONObject?): String {
        val filed = reports?.optJSONArray("reports")?.length() ?: 0
        val phone = "$filed report${if (filed == 1) "" else "s"} filed from this phone."
        return when (playtest?.optString("state")) {
            "connecting", "reading" -> "$phone Reading playtest awards…"
            else -> phone
        }
    }
}
