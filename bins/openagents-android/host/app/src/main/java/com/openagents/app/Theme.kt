package com.openagents.app

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject

/**
 * The app's palette, from the theme Rust resolved (#11028): Rust owns the
 * choice (Account > Appearance: System, Light, Dark) and resolves it against
 * the phone's appearance, which this host reports. The values start as the
 * dark look (white text on black, gray surfaces) and change when a packet's
 * `appearance` does ([apply]); views read them when they are built.
 */
internal object Palette {
    var BACKGROUND = 0xFF000000.toInt(); private set
    var PRIMARY = 0xFFFFFFFF.toInt(); private set
    var SECONDARY = 0xFF9A9AA0.toInt(); private set
    var TERTIARY = 0xFF5C5C62.toInt(); private set
    var BUBBLE = 0xFF292929.toInt(); private set
    var SURFACE = 0xFF141414.toInt(); private set
    var RAISED = 0xFF212121.toInt(); private set
    var BORDER = 0xFF383838.toInt(); private set
    var INLINE_CODE = 0x1AFFFFFF; private set
    var LINK = 0xFF0A84FF.toInt(); private set
    var SUCCESS = 0xFF30D158.toInt(); private set
    var FAILURE = 0xFFFF453A.toInt(); private set
    /** Selected text's highlight and the selection handles, as on iOS. */
    var SELECTION = 0x4D0A84FF; private set
    var SELECTION_HANDLE = 0xFF0A84FF.toInt(); private set
    /** Whether the resolved theme is light: dark status bar icons on a light bar. */
    var LIGHT = false; private set
    /** The widest a transcript row or composer grows, centered beyond it. */
    const val READING_WIDTH_DP = 720

    /** The `appearance` Rust last sent, as applied. */
    private var applied: String? = null

    /**
     * Takes Rust's `appearance` (`{scheme, palette: {background: {red, green,
     * blue, alpha}, ...}}`). Answers whether the colors changed, so the host
     * rebuilds what it drew.
     */
    fun apply(appearance: JSONObject?): Boolean {
        val palette = appearance?.optJSONObject("palette") ?: return false
        val encoded = palette.toString() + appearance.optString("scheme")
        if (encoded == applied) return false
        applied = encoded
        fun color(name: String, fallback: Int): Int = palette.optJSONObject(name)?.let {
            android.graphics.Color.argb(it.optInt("alpha", 255), it.optInt("red"), it.optInt("green"), it.optInt("blue"))
        } ?: fallback
        BACKGROUND = color("background", BACKGROUND)
        PRIMARY = color("primary", PRIMARY)
        SECONDARY = color("secondary", SECONDARY)
        TERTIARY = color("tertiary", TERTIARY)
        BUBBLE = color("bubble", BUBBLE)
        SURFACE = color("surface", SURFACE)
        RAISED = color("raised", RAISED)
        BORDER = color("border", BORDER)
        INLINE_CODE = color("inline_code", INLINE_CODE)
        LINK = color("link", LINK)
        SUCCESS = color("success", SUCCESS)
        FAILURE = color("failure", FAILURE)
        SELECTION = color("selection", SELECTION)
        SELECTION_HANDLE = color("selection_handle", SELECTION_HANDLE)
        LIGHT = appearance.optString("scheme") == "light"
        return true
    }
}

/**
 * Coder Noir for terminal panes, which stay dark in both themes like every
 * terminal UI.
 */
internal object Noir {
    const val BACKGROUND = 0xFF000000.toInt()
}

internal fun Context.dp(value: Int) = (value * resources.displayMetrics.density).toInt()
internal fun Context.dpf(value: Float) = value * resources.displayMetrics.density

internal fun Context.text(value: String, size: Float = 16f, color: Int = Palette.PRIMARY): TextView =
    TextView(this).apply {
        text = value; textSize = size; setTextColor(color); isSaveEnabled = false
    }

internal fun Context.column(): LinearLayout = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
internal fun Context.row(): LinearLayout = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }

/** A filled rounded rectangle with an optional 1dp border. Radii are top-left, top-right, bottom-right, bottom-left. */
internal fun Context.rounded(fill: Int, radius: Float, border: Int? = null,
                             radii: FloatArray? = null): GradientDrawable = GradientDrawable().apply {
    setColor(fill)
    if (radii != null) cornerRadii = radii.flatMap { listOf(dpf(it), dpf(it)) }.toFloatArray()
    else cornerRadius = dpf(radius)
    if (border != null) setStroke(dp(1), border)
}

internal fun View.keyed(key: String): View { tag = key; return this }
