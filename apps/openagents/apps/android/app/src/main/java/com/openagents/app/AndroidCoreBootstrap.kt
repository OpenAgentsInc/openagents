package com.openagents.app

import android.content.Context
import uniffi.openagents_core.UniffiLib

/** Installs the Android runtime services that the Rust core needs before networking starts. */
object AndroidCoreBootstrap {
    private val state = BootstrapState()

    @JvmStatic
    fun initialize(context: Context) {
        val applicationContext = requireApplicationContext(context)
        state.initialize {
            UniffiLib.ensureInitialized()
            nativeInitialize(applicationContext)
        }
    }

    @JvmStatic
    fun requireInitialized() = state.requireInitialized()

    @JvmStatic
    private external fun nativeInitialize(context: Context)
}

internal fun requireApplicationContext(context: Context?): Context =
    requireNotNull(context?.applicationContext) {
        "Android application context is unavailable. Restart the app and try again."
    }

internal class BootstrapState {
    private var initialized = false

    @Synchronized
    fun initialize(install: () -> Unit) {
        if (initialized) return
        install()
        initialized = true
    }

    @Synchronized
    fun requireInitialized() {
        check(initialized) {
            "App startup did not finish. Restart the app and try again."
        }
    }
}
