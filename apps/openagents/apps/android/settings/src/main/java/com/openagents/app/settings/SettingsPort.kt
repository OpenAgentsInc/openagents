package com.openagents.app.settings

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.openagents_core.CoreSettingsInterface
import uniffi.openagents_core.SettingsEffect

/** Identity-only native events. This port never accepts a deletion/forfeit flag. */
interface SettingsPort {
    suspend fun snapshot(): ByteArray
    fun acknowledge(instance: String, revision: ULong)
    suspend fun activate(instance: String, revision: ULong, key: String): SettingsEffect
    fun changeField(instance: String, revision: ULong, key: String, value: String)
    suspend fun refreshCloud()
    suspend fun wallpaper(ticket: String, uri: String, name: String, permissionSaved: Boolean): SettingsEffect
    fun dispose()
}

class RustSettingsPort(private val core: CoreSettingsInterface) : SettingsPort {
    override suspend fun snapshot() = withContext(Dispatchers.Default) { core.snapshot() }
    override fun acknowledge(instance: String, revision: ULong) = core.acknowledge(instance, revision)
    override suspend fun activate(instance: String, revision: ULong, key: String) = core.activate(instance, revision, key)
    override fun changeField(instance: String, revision: ULong, key: String, value: String) = core.changeField(instance, revision, key, value)
    override suspend fun refreshCloud() = core.refreshCloud()
    override suspend fun wallpaper(ticket: String, uri: String, name: String, permissionSaved: Boolean) =
        core.wallpaperSelected(ticket, uri, name, permissionSaved)
    override fun dispose() = core.dispose()
}
