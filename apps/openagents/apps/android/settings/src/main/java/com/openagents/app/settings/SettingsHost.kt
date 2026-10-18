package com.openagents.app.settings

import android.content.Context
import android.view.ViewGroup
import com.openagents.app.SemanticViewAdapter
import kotlinx.coroutines.CoroutineScope
import uniffi.openagents_core.SettingsEffect

/** One account generation and surface lifetime. Never save its JSON in a Bundle. */
class SettingsHost(
    context: Context,
    container: ViewGroup,
    core: SettingsPort,
    scope: CoroutineScope,
    effects: (SettingsEffect) -> Unit,
    unavailable: () -> Unit,
) : AutoCloseable {
    private lateinit var presenter: SettingsPresenter
    private val adapter = SemanticViewAdapter(context,
        activate = { instance, revision, key -> presenter.activate(instance, revision.toULong(), key) },
        changeField = { instance, revision, key, value -> presenter.change(instance, revision.toULong(), key, value) },
        acknowledge = { instance, revision -> core.acknowledge(instance, revision.toULong()) },
    )
    init {
        presenter = SettingsPresenter(core, scope, { adapter.mount(container, it) }, effects, {
            presenter.dispose()
            adapter.dispose()
            unavailable()
        })
    }
    /** Wire SettingsListener here on the UI thread; the shell provides a UI scope. */
    fun refresh() = presenter.refresh()
    fun start() { presenter.refresh(); presenter.refreshCloud() }
    fun wallpaper(ticket: String, uri: String, name: String, permissionSaved: Boolean) =
        presenter.wallpaper(ticket, uri, name, permissionSaved)
    override fun close() { presenter.dispose(); adapter.dispose() }
}
