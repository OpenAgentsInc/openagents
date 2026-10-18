package com.openagents.app

import android.os.Handler
import android.os.Looper
import com.openagents.app.onboarding.RecoveryPort
import uniffi.openagents_core.AndroidWalletFrame
import uniffi.openagents_core.CoreAndroidWallet
import uniffi.openagents_core.RecoveryFrame
import java.util.concurrent.atomic.AtomicBoolean

/** Reuses the credential/secret-screen host; wallet words and backup actions remain Rust-owned. */
internal class AndroidWalletRecoveryPort(
    private val core: CoreAndroidWallet,
    private val worker: (() -> Unit) -> Unit,
    private val redraw: () -> Unit,
) : RecoveryPort {
    private val main = Handler(Looper.getMainLooper())
    private val closed = AtomicBoolean(false)
    private var expiry: Runnable? = null
    private fun frame(value: AndroidWalletFrame): RecoveryFrame {
        main.post {
            if (closed.get()) return@post
            expiry?.let { main.removeCallbacks(it) }; expiry = null
            value.redactAfterMs?.let { delay ->
                val task = Runnable {
                    if (!closed.get()) worker {
                        core.redact()
                        main.post { if (!closed.get()) redraw() }
                    }
                }
                expiry = task
                main.postDelayed(task, delay.toLong())
            }
        }
        return RecoveryFrame(value.json, value.secure, value.authentication, false, value.clipboard)
    }
    override fun start(device: String) = frame(core.open())
    override fun render() = frame(core.snapshot())
    override fun acknowledge(instance: String, revision: Long) = core.acknowledge(instance, revision.toULong())
    override fun activate(instance: String, revision: Long, key: String) = frame(core.activate(instance, revision.toULong(), key))
    override fun change(instance: String, revision: Long, key: String, value: String) = core.changeField(instance, revision.toULong(), key, value)
    override fun authenticated(token: ULong, accepted: Boolean) = frame(core.authenticated(token, accepted))
    override fun redact() = core.redact()
    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        main.removeCallbacksAndMessages(null)
        core.redact()
        core.destroy()
    }
}
