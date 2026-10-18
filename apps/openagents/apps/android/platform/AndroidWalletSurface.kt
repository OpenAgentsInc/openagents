package com.openagents.app.wallet

import android.content.Context
import android.view.ViewGroup
import com.openagents.app.rustnative.Activation
import com.openagents.app.rustnative.AndroidRustNativeViewAdapter
import com.openagents.app.rustnative.FieldChange
import com.openagents.app.rustnative.MountedView
import com.openagents.app.rustnative.RustNativeViewCallbacks
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
) : RustNativeViewCallbacks, AutoCloseable {
    private val wallet = CoreWallet.android(
        home,
        storage,
        retiredSeed,
        object : WalletListener {
            override fun onWalletChanged() {
                listener.onWalletChanged()
                render()
            }
        },
    )
    private val adapter = AndroidRustNativeViewAdapter(context, this)
    private var target: ViewGroup? = null
    private var mounted: MountedView? = null

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

    override fun activate(activation: Activation) {
        onEffect(wallet.activate(activation.instance, activation.revision, activation.node))
        render()
    }

    override fun changeField(change: FieldChange) {
        val activation = change.activation
        wallet.changeField(activation.instance, activation.revision, activation.node, change.value)
        render()
    }

    private fun render() {
        val host = target ?: return
        mounted?.close()
        mounted = adapter.mount(wallet.snapshot(), host)
    }

    override fun close() {
        mounted?.close()
        mounted = null
        target = null
        wallet.close()
    }
}
