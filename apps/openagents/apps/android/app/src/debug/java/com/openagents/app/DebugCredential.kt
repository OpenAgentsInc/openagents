package com.openagents.app

import java.io.File

/**
 * Debug-only sign-in for live checks with a dedicated test account. A check
 * writes the credential with `adb shell run-as` into the app's private files;
 * the app reads it once, deletes it, and hands it to Rust. This file is absent
 * from release.
 */
internal object DebugCredential {
    const val FILE = "debug-credential"

    fun take(filesDir: File): String? {
        val file = File(filesDir, FILE)
        if (!file.isFile || file.length() > 4096) {
            file.delete()
            return null
        }
        val credential = runCatching { file.readText().trim() }.getOrNull()
        file.delete()
        return credential?.takeIf { it.isNotEmpty() }
    }
}
