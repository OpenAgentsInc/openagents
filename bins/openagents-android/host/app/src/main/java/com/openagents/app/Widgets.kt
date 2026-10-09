// Small white-on-black building blocks the host's own screens share: the
// Wallet, the Account screens, and the Verse panels. Rust decides every
// value and line of text they show.
package com.openagents.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView

/** A text label; `mono` draws it in the system monospace face, for keys and identifiers. */
internal fun Context.label(value: String, size: Float = 14f, color: Int = Palette.PRIMARY, key: String? = null,
                           bold: Boolean = false, mono: Boolean = false, selectable: Boolean = false): TextView =
    text(value, size, color).apply {
        if (mono) typeface = Fonts.code(context, if (bold) Fonts.BOLD else Fonts.REGULAR)
        else if (bold) typeface = Fonts.typeface(context, Fonts.BOLD)
        if (selectable) setTextIsSelectable(true)
        key?.let { tag = it }
    }

/** A rounded, outlined button; `primary` fills it white with black text. */
internal fun Context.pill(value: String, key: String? = null, primary: Boolean = false, action: () -> Unit): TextView =
    text(value, 15f, if (primary) Palette.BACKGROUND else Palette.PRIMARY).apply {
        gravity = Gravity.CENTER
        minHeight = dp(44)
        setPadding(dp(16), dp(10), dp(16), dp(10))
        background = if (primary) rounded(Palette.PRIMARY, 22f) else rounded(Palette.RAISED, 22f, Palette.BORDER)
        if (primary) typeface = Fonts.typeface(context, Fonts.BOLD)
        isClickable = true; isFocusable = true
        key?.let { tag = it }
        setOnClickListener { if (isEnabled) action() }
    }

/** Enables or disables a view built by [pill], dimming it when off. */
internal fun <T : View> T.enabled(value: Boolean): T { isEnabled = value; alpha = if (value) 1f else 0.4f; return this }

internal fun Context.divider(): View = View(this).apply { setBackgroundColor(Palette.BORDER) }

internal fun LinearLayout.add(view: View, top: Int = 0, width: Int = -1): View {
    addView(view, LinearLayout.LayoutParams(width, -2).apply { topMargin = context.dp(top) })
    return view
}

internal fun LinearLayout.addDivider(top: Int = 10) {
    addView(context.divider(), LinearLayout.LayoutParams(-1, 1).apply { topMargin = context.dp(top) })
}

/** Labels as outlined chips that wrap to new rows. */
internal fun Context.chips(chips: List<Pair<String, String>>, small: Boolean = false): View = FlowLayout(this, dp(4)).apply {
    for ((code, value) in chips) addView(text(value, if (small) 11f else 12f).apply {
        setPadding(dp(if (small) 5 else 7), dp(2), dp(if (small) 5 else 7), dp(2))
        val strong = code == "thin_margin" || code == "in_sample"
        background = rounded(0, 10f, if (strong) 0xE6FFFFFF.toInt() else 0x66FFFFFF)
    })
    contentDescription = chips.joinToString(", ") { it.second }
}

internal fun org.json.JSONArray?.chipList(): List<Pair<String, String>> =
    this?.objects()?.map { it.getString("code") to it.getString("text") } ?: emptyList()

internal fun org.json.JSONArray?.strings(): List<String> =
    this?.let { list -> (0 until list.length()).map { list.getString(it) } } ?: emptyList()

/** Lays out its children left to right, wrapping to new rows. */
internal class FlowLayout(context: Context, private val spacing: Int) : ViewGroup(context) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec).takeIf { MeasureSpec.getMode(widthMeasureSpec) != MeasureSpec.UNSPECIFIED }
            ?: Int.MAX_VALUE
        var x = 0; var y = 0; var row = 0; var widest = 0
        for (i in 0 until childCount) {
            val child = getChildAt(i)
            child.measure(MeasureSpec.makeMeasureSpec(width, MeasureSpec.AT_MOST), MeasureSpec.UNSPECIFIED)
            if (x > 0 && x + child.measuredWidth > width) { x = 0; y += row + spacing; row = 0 }
            x += child.measuredWidth + spacing
            row = maxOf(row, child.measuredHeight)
            widest = maxOf(widest, x - spacing)
        }
        setMeasuredDimension(if (width == Int.MAX_VALUE) widest else resolveSize(widest, widthMeasureSpec), y + row)
    }

    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        val width = r - l
        var x = 0; var y = 0; var row = 0
        for (i in 0 until childCount) {
            val child = getChildAt(i)
            if (x > 0 && x + child.measuredWidth > width) { x = 0; y += row + spacing; row = 0 }
            child.layout(x, y, x + child.measuredWidth, y + child.measuredHeight)
            x += child.measuredWidth + spacing
            row = maxOf(row, child.measuredHeight)
        }
    }
}

/**
 * Copies a value. A secret is marked sensitive, so the keyboard's clipboard
 * preview hides it (Android 13 and later), and this app clears it from the
 * clipboard after a minute if it is still there.
 */
internal object Clipboard {
    private val main = Handler(Looper.getMainLooper())

    fun copy(context: Context, label: String, value: String, secret: Boolean = false) {
        val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return
        val clip = ClipData.newPlainText(label, value)
        if (secret) clip.description.extras = PersistableBundle().apply {
            putBoolean(if (Build.VERSION.SDK_INT >= 33) ClipDescription.EXTRA_IS_SENSITIVE else "android.content.extra.IS_SENSITIVE", true)
        }
        clipboard.setPrimaryClip(clip)
        if (secret) main.postDelayed({
            val current = clipboard.primaryClip
            if (current != null && current.itemCount > 0 && current.getItemAt(0).text?.toString() == value) {
                if (Build.VERSION.SDK_INT >= 28) clipboard.clearPrimaryClip()
                else clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
            }
        }, 60_000)
    }
}
