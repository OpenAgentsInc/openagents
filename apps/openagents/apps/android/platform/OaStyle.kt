package com.openagents.app

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.view.Gravity
import android.view.View
import android.widget.ImageButton
import android.widget.TextView

/** Shared presentation values for the native shell: the oa_* palette, Geist, and rounded shapes. */
object OaStyle {
    enum class Weight(val asset: String) {
        REGULAR("fonts/Geist.ttf"),
        MEDIUM("fonts/Geist-Medium.ttf"),
        SEMIBOLD("fonts/Geist-SemiBold.ttf"),
        BOLD("fonts/Geist-Bold.ttf"),
    }

    private val faces = mutableMapOf<Weight, Typeface>()

    /** Geist from the bundled transcript fonts, falling back to the system face. */
    fun typeface(context: Context, weight: Weight = Weight.REGULAR): Typeface = faces.getOrPut(weight) {
        runCatching { Typeface.createFromAsset(context.applicationContext.assets, weight.asset) }
            .getOrElse { if (weight == Weight.REGULAR) Typeface.DEFAULT else Typeface.DEFAULT_BOLD }
    }

    fun dp(context: Context, value: Float): Int = (value * context.resources.displayMetrics.density + 0.5f).toInt()
    fun dp(context: Context, value: Int): Int = dp(context, value.toFloat())
    fun color(context: Context, resource: Int): Int = context.getColor(resource)

    /** Sets size, weight, and color in one place so every surface reads the same. */
    fun text(view: TextView, sizeSp: Float, weight: Weight = Weight.REGULAR, colorResource: Int = R.color.oa_text) {
        view.typeface = typeface(view.context, weight)
        view.textSize = sizeSp
        view.setTextColor(view.context.getColor(colorResource))
        view.includeFontPadding = false
    }

    fun rounded(context: Context, fill: Int, radiusDp: Float, stroke: Int? = null): GradientDrawable =
        GradientDrawable().apply {
            setColor(fill)
            cornerRadius = dp(context, radiusDp).toFloat()
            stroke?.let { setStroke(dp(context, 1f).coerceAtLeast(1), it) }
        }

    fun circle(fill: Int, stroke: Int? = null, strokeWidth: Int = 1): GradientDrawable = GradientDrawable().apply {
        shape = GradientDrawable.OVAL
        setColor(fill)
        stroke?.let { setStroke(strokeWidth, it) }
    }

    /** Wraps a background in the platform press ripple, clipped to the same shape. */
    fun pressable(context: Context, content: Drawable?, mask: Drawable? = content): Drawable =
        RippleDrawable(ColorStateList.valueOf(context.getColor(R.color.oa_hairline)), content, mask)

    /** A borderless pressable row background with rounded corners. */
    fun rowBackground(context: Context, fill: Int, radiusDp: Float): Drawable =
        pressable(context, rounded(context, fill, radiusDp), rounded(context, 0xFF000000.toInt(), radiusDp))

    /** A round icon button with an optional filled disc behind the glyph. */
    fun iconButton(
        context: Context,
        icon: Int,
        label: String,
        sizeDp: Int = 44,
        fill: Int? = null,
        stroke: Int? = null,
        tint: Int = context.getColor(R.color.oa_text),
    ): ImageButton = ImageButton(context).apply {
        setImageResource(icon)
        imageTintList = ColorStateList.valueOf(tint)
        contentDescription = label
        tooltipText = label
        scaleType = android.widget.ImageView.ScaleType.CENTER
        val disc = circle(fill ?: 0, stroke, dp(context, 1f).coerceAtLeast(1))
        background = pressable(context, if (fill != null || stroke != null) disc else null, circle(0xFF000000.toInt()))
        minimumWidth = dp(context, sizeDp)
        minimumHeight = dp(context, sizeDp)
        setPadding(0, 0, 0, 0)
        foregroundGravity = Gravity.CENTER
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
    }
}
