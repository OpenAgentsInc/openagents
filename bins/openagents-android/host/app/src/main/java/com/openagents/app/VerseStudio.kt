// Everglade's Agent Studio on the Verse tab, as in the iOS Verse tab: one
// line about the studio's connection to a paired computer, Interact while a
// station is in reach, and the station's panel, which Rust draws as a Rust
// Native view, with a field whose text Rust sends as the panel says. Rust
// owns the panel, the grant's rights, and every intent; this only connects
// the studio, mounts the view, and forwards activations and text.
package com.openagents.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import org.json.JSONObject

class VerseStudio(
    private val context: Context,
    private val world: VerseSurface,
    /** The paired computer the studio acts through, as its host key and name. */
    private val computer: () -> Pair<String, String>?,
    /** Takes the app's link to a computer on the app worker (`MobileBridge.studioLinks`). */
    private val links: (String, (Long?) -> Unit) -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())
    /** Fills the Verse page; only its children take touches. */
    val root = FrameLayout(context)
    private val panel = context.column().apply {
        setPadding(context.dp(12), context.dp(10), context.dp(12), context.dp(12))
        background = context.rounded(0xF70A0A0A.toInt(), 16f, 0x8CFFFFFF.toInt())
        isClickable = true // Touches on the panel stay off the world.
        visibility = View.GONE
        tag = "verse-studio-panel"
    }
    private val content = FrameLayout(context)
    private val renderer = NativeRenderer(context, { view, node -> activate(view, node) })
    private val loading = context.label("Loading studio…", 14f, Palette.SECONDARY, key = "verse-studio-loading")
    private val error = context.label("", 13f, Palette.SECONDARY, key = "verse-studio-error").apply { visibility = View.GONE }
    private val field = EditText(context).apply {
        hint = "Answer or message"
        setTextColor(Palette.PRIMARY); setHintTextColor(Palette.SECONDARY)
        inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
        maxLines = 4
        background = context.rounded(Palette.RAISED, 10f, Palette.BORDER)
        setPadding(context.dp(10), context.dp(8), context.dp(10), context.dp(8))
        tag = "verse-studio-text"
    }
    private val send = context.pill("Send", "verse-studio-send", primary = true) { sendText() }
    private val interact = context.pill("Interact", "verse-interact") {
        world.send(json("action" to "zone", "intent" to "interact"))
    }.apply { visibility = View.GONE }
    private val statusText = context.label("", 12f, Palette.SECONDARY, key = "verse-studio-status")
    private val retry = context.label("Try again", 12f, Palette.PRIMARY, key = "verse-studio-retry", bold = true).apply {
        setPadding(context.dp(10), 0, 0, 0)
        setOnClickListener { attempt = null; failed = false; sync() }
    }
    private val status = context.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        background = context.rounded(0x8C000000.toInt(), 14f)
        setPadding(context.dp(12), context.dp(6), context.dp(12), context.dp(6))
        visibility = View.GONE
        addView(statusText); addView(retry)
    }

    private var open = false
    private var view: JSONObject? = null
    private var requested = -1L
    private var interactLabel: String? = null
    private var inEverglade = false
    private var statusLine: String? = null
    private var failed = false
    /** The computer and world handle the studio connected through, or last failed with. */
    private var attempt: String? = null
    private var connecting = false
    private var insetTop = 0
    private var boardOpen = false

    /** The studio panel covers the world. */
    val showing get() = open

    init {
        val composer = context.row().apply { gravity = Gravity.BOTTOM }
        composer.addView(field, LinearLayout.LayoutParams(0, -2, 1f))
        composer.addView(send, LinearLayout.LayoutParams(-2, -2).apply { marginStart = context.dp(8) })
        panel.addView(content, LinearLayout.LayoutParams(-1, 0, 1f))
        panel.addView(error, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(6) })
        panel.addView(composer, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(8) })
        root.addView(panel, FrameLayout.LayoutParams(-1, -1))
        root.addView(status, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL))
        root.addView(interact, FrameLayout.LayoutParams(-2, -2, Gravity.END or Gravity.CENTER_VERTICAL).apply {
            marginEnd = context.dp(16) })
        place()
    }

    fun setTopInset(value: Int) { if (insetTop != value) { insetTop = value; place() } }

    private fun place() {
        val margin = context.dp(12)
        (panel.layoutParams as FrameLayout.LayoutParams).apply {
            setMargins(margin, insetTop + margin, margin, context.dp(16))
            panel.layoutParams = this
        }
        (status.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = insetTop + margin
            status.layoutParams = this
        }
    }

    /** The system Back gesture closes the panel. */
    fun back() { world.send(json("action" to "close_studio")) }

    /** Applies a world packet. */
    fun update(packet: JSONObject?) {
        packet ?: return
        val zone = packet.objectOrNull("zone")
        val everglade = zone != null && zone.optString("id") == "everglade" && zone.optString("state") == "idle"
        interactLabel = zone?.optJSONArray("controls")?.objects()
            ?.firstOrNull { it.optString("action") == "interact" && it.optBoolean("enabled") }
            ?.optString("label")
        val nextOpen = packet.optBoolean("studio_open")
        if (!nextOpen) { view = null; requested = -1 }
        else packet.objectOrNull("studio_view")?.takeIf { it.optString("schema") == "rust-native.view.v2" }
            ?.let { view = it; requested = it.optLong("revision") }
        // Rust rebuilds the panel as the studio changes; a frame carries only
        // its revision, so ask for the view when it changed.
        if (nextOpen) {
            val revision = packet.optLong("studio_revision", -1)
            if (revision >= 0 && view?.optLong("revision") != revision && requested != revision) {
                requested = revision
                main.post { if (open) world.send(json("action" to "studio_view")) }
            }
        }
        if (nextOpen != open) {
            open = nextOpen
            panel.visibility = if (open) View.VISIBLE else View.GONE
            if (!open) {
                renderer.clear(); content.removeAllViews(); field.setText("")
                context.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(field.windowToken, 0)
            }
        }
        if (open) {
            val shown = view
            try {
                if (shown != null) renderer.mount(content, shown)
                else if (loading.parent == null) { renderer.clear(); content.removeAllViews(); content.addView(loading) }
            } catch (failure: Exception) {
                renderer.clear(); content.removeAllViews()
                content.addView(context.label(failure.message ?: "This panel couldn't be shown.", 14f, Palette.SECONDARY))
            }
            val problem = packet.textOrNull("error")
            error.text = problem ?: ""
            error.visibility = if (problem == null) View.GONE else View.VISIBLE
        }
        if (everglade != inEverglade) { inEverglade = everglade; sync() }
        render()
    }

    /** Whether the Gym's boards' panels are open, which hide the studio's controls. */
    fun setBoardOpen(value: Boolean) { if (boardOpen != value) { boardOpen = value; render() } }

    private fun render() {
        val label = interactLabel
        interact.visibility = if (label != null && !open && !boardOpen) View.VISIBLE else View.GONE
        if (label != null) {
            interact.text = label
            interact.contentDescription = "Interact: $label. Opens this station's studio panel."
        }
        val line = statusLine
        status.visibility = if (inEverglade && line != null && !open && !boardOpen) View.VISIBLE else View.GONE
        statusText.text = line ?: ""
        retry.visibility = if (failed) View.VISIBLE else View.GONE
    }

    /**
     * Connects Everglade's studio to the paired computer while the player is
     * in Everglade, once per computer and world. Rust answers the grant's
     * rights, or why it could not connect.
     */
    fun sync() {
        val handle = world.handleId
        if (!inEverglade || connecting || handle == 0L) return
        val target = computer()
        if (target == null) {
            if (attempt == null) say("No computer is online for the studio. Check Account > Computers.", false)
            return
        }
        val key = "${target.first}@$handle"
        if (key == attempt) return
        attempt = key; connecting = true
        say("Connecting the studio to ${target.second}…", false)
        links(target.first) { token ->
            connecting = false
            val reply = token?.let { world.connectStudio(it) }
            if (reply?.optBoolean("connected") == true) {
                val rights = reply.optJSONArray("rights")?.let { list ->
                    (0 until list.length()).joinToString(", ") { list.getString(it) }
                }.orEmpty()
                say("Studio on ${target.second}" + if (rights.isEmpty()) "" else ": $rights", false)
            } else {
                say(reply?.textOrNull("error") ?: "The studio couldn't connect to ${target.second}.", true)
            }
        }
    }

    private fun say(line: String, failure: Boolean) { statusLine = line; failed = failure; render() }

    private fun activate(view: JSONObject, node: String) {
        world.send(json("action" to "studio_activate", "instance" to view.getString("instance"),
            "revision" to view.getLong("revision"), "node" to node))
    }

    private fun sendText() {
        val text = field.text.toString().trim()
        if (text.isEmpty() || text.toByteArray().size > 60_000) return
        val result = world.send(json("action" to "studio_text", "text" to text))
        if (result != null && result.textOrNull("error") == null) field.setText("")
    }
}
