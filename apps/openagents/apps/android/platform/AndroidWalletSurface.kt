package com.openagents.app.wallet

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.AndroidRetiredWalletSeed
import uniffi.openagents_core.AndroidSecretStorage
import uniffi.openagents_core.CoreWallet
import uniffi.openagents_core.WalletListener

/** Connects the Rust wallet view and actions to the shared Android Rust Native adapter. */
class AndroidWalletSurface(
    context: Context,
    home: String,
    storage: AndroidSecretStorage,
    retiredSeed: AndroidRetiredWalletSeed,
    listener: WalletListener,
    private val onEffect: (String) -> Unit,
) : AutoCloseable {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var closed = false
    private val wallet = CoreWallet.android(
        home,
        storage,
        retiredSeed,
        object : WalletListener {
            override fun onWalletChanged() {
                mainHandler.post {
                    if (closed) return@post
                    listener.onWalletChanged()
                    render()
                }
            }
        },
    )
    private val adapter = SemanticViewAdapter(
        context,
        activate = { instance, revision, key ->
            require(revision >= 0)
            onEffect(wallet.activate(instance, revision.toULong(), key))
            render()
        },
        changeField = { instance, revision, key, value ->
            require(revision >= 0)
            wallet.changeField(instance, revision.toULong(), key, value)
            render()
        },
        // Wallet snapshots are validated directly by CoreWallet; they have no
        // separate Rust Native delivery acknowledgment.
        acknowledge = { _, _ -> },
    )
    private var target: ViewGroup? = null

    fun open(target: ViewGroup) {
        this.target = target
        wallet.open()
        render()
    }

    /** Call only after Android confirms the person's identity. */
    fun recoveryWordsAfterAuthentication(): List<String> = wallet.recoveryWords()

    fun markRecoveryWordsSaved() {
        wallet.wordsSaved()
        render()
    }

    private fun render() {
        val host = target ?: return
        adapter.mount(host, wallet.snapshot().toByteArray(Charsets.UTF_8))
    }

    override fun close() {
        if (closed) return
        closed = true
        mainHandler.removeCallbacksAndMessages(null)
        adapter.dispose()
        target = null
    }
}
