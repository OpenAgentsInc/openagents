package com.openagents.app

import java.io.File

/**
 * Debug-only sign-in for live checks with a dedicated test account. A check
 * writes the credential with `adb shell run-as` into the app's private files;
 * the app reads it once, deletes it, and hands it to Rust. Later launches
 * restore that sign-in without the install's own key, so the key is never
 * linked to the shared test account. This file is absent from release.
 */
internal object DebugCredential {
    const val FILE = "debug-credential"
    private const val MODE = "debug-credential-mode"

    fun take(filesDir: File): String? {
        val file = File(filesDir, FILE)
        if (!file.isFile || file.length() > 4096) {
            file.delete()
            return null
        }
        val credential = runCatching { file.readText().trim() }.getOrNull()
        file.delete()
        return credential?.takeIf { it.isNotEmpty() }?.also { File(filesDir, MODE).writeText("1") }
    }

    /** True after a credential sign-in, until the app's data is cleared. */
    fun credentialMode(filesDir: File): Boolean = File(filesDir, MODE).isFile
}
