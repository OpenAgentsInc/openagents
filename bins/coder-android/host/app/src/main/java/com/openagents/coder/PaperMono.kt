package com.openagents.coder

import android.content.Context
import android.graphics.Typeface
import android.os.Build

/**
 * Paper Mono, the one typeface every OpenAgents surface uses. Every typeface
 * the app sets goes through [typeface]; the theme sets the same family for
 * views that set none.
 */
internal object PaperMono {
    const val REGULAR = 400
    const val MEDIUM = 500
    const val SEMIBOLD = 600
    const val BOLD = 700

    private val faces = HashMap<Int, Typeface>()

    /**
     * Paper Mono at [weight]. Weights below 400 draw Regular and above 700
     * draw Bold. Regular is the whole family, so a bold span selects the Bold
     * face rather than a synthesized one. Paper Mono has no italic.
     */
    @Synchronized fun typeface(context: Context, weight: Int = REGULAR): Typeface {
        val nearest = when {
            weight < 450 -> REGULAR
            weight < 550 -> MEDIUM
            weight < 650 -> SEMIBOLD
            else -> BOLD
        }
        return faces.getOrPut(nearest) {
            val family = context.resources.getFont(R.font.paper_mono)
            when {
                nearest == REGULAR -> family
                Build.VERSION.SDK_INT >= 28 -> Typeface.create(family, nearest, false)
                nearest == BOLD -> Typeface.create(family, Typeface.BOLD)
                nearest == MEDIUM -> context.resources.getFont(R.font.paper_mono_medium)
                else -> context.resources.getFont(R.font.paper_mono_semibold)
            }
        }
    }
}
