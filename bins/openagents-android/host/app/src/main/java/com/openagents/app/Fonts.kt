package com.openagents.app

import android.content.Context
import android.graphics.Typeface
import android.os.Build

/**
 * The app's type: the web's (#11120). Text draws in the system sans-serif
 * face (Roboto), the `sans-serif` family of the web's `--font-sans` stack,
 * and code in the system `monospace` face of `--font-mono`. The app bundles
 * no font; the theme sets the same sans-serif family for views that set
 * none. crates/openagents-mobile/src/typeface_tests.rs holds this file and
 * the theme to the token stacks.
 */
internal object Fonts {
    const val REGULAR = 400
    const val MEDIUM = 500
    const val SEMIBOLD = 600
    const val BOLD = 700

    private val faces = HashMap<Pair<Int, Boolean>, Typeface>()

    /** The system sans-serif face at [weight]. */
    @Synchronized fun typeface(@Suppress("UNUSED_PARAMETER") context: Context, weight: Int = REGULAR): Typeface =
        face(weight, mono = false)

    /** The system monospace face at [weight], for code, keys, and identifiers. */
    @Synchronized fun code(@Suppress("UNUSED_PARAMETER") context: Context, weight: Int = REGULAR): Typeface =
        face(weight, mono = true)

    private fun face(weight: Int, mono: Boolean): Typeface = faces.getOrPut(weight to mono) {
        val family = if (mono) Typeface.MONOSPACE else Typeface.SANS_SERIF
        when {
            Build.VERSION.SDK_INT >= 28 -> Typeface.create(family, weight.coerceIn(1, 1000), false)
            weight >= SEMIBOLD -> Typeface.create(family, Typeface.BOLD)
            else -> family
        }
    }
}
