package com.openagents.app

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView

/** OpenAgents' neutral dark palette: white text on black, gray surfaces. */
internal object Palette {
    const val BACKGROUND = 0xFF000000.toInt()
    const val PRIMARY = 0xFFFFFFFF.toInt()
    const val SECONDARY = 0xFF9A9AA0.toInt()
    const val TERTIARY = 0xFF5C5C62.toInt()
    const val BUBBLE = 0xFF292929.toInt()
    const val SURFACE = 0xFF141414.toInt()
    const val RAISED = 0xFF212121.toInt()
    const val BORDER = 0xFF383838.toInt()
    const val INLINE_CODE = 0x1AFFFFFF
    const val LINK = 0xFF0A84FF.toInt()
    const val SUCCESS = 0xFF30D158.toInt()
    const val FAILURE = 0xFFFF453A.toInt()
    /** Selected text's highlight and the selection handles, as on iOS. */
    const val SELECTION = 0x4D0A84FF
    const val SELECTION_HANDLE = 0xFF0A84FF.toInt()
    /** The widest a transcript row or composer grows, centered beyond it. */
    const val READING_WIDTH_DP = 720
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
