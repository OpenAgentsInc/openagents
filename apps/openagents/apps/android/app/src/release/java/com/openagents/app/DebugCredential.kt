package com.openagents.app

import java.io.File

/** Release never reads a local credential file. */
internal object DebugCredential {
    fun take(@Suppress("UNUSED_PARAMETER") filesDir: File): String? = null
}
