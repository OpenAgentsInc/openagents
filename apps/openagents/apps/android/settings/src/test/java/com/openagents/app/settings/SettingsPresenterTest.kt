package com.openagents.app.settings

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.yield
import org.junit.Assert.*
import org.junit.Test
import uniffi.openagents_core.SettingsEffect

class SettingsPresenterTest {
    /** These are opaque synthetic frames, not runtime captures or Android output. */
    private open class Fixture : SettingsPort {
        val events = mutableListOf<Triple<String, ULong, String>>()
        val fields = mutableListOf<String>()
        val acknowledgments = mutableListOf<Pair<String, ULong>>()
        val wallpapers = mutableListOf<List<Any>>()
        var reads = 0
        var disposed = false
        override suspend fun snapshot(): ByteArray { reads++; return byteArrayOf(reads.toByte()) }
        override fun acknowledge(instance: String, revision: ULong) { acknowledgments += instance to revision }
        override suspend fun activate(instance: String, revision: ULong, key: String): SettingsEffect {
            events += Triple(instance, revision, key)
            return SettingsEffect.None
        }
        override fun changeField(instance: String, revision: ULong, key: String, value: String) { fields += value }
        override suspend fun refreshCloud() = Unit
        override suspend fun wallpaper(ticket: String, uri: String, name: String, permissionSaved: Boolean): SettingsEffect {
            wallpapers += listOf(ticket, uri, name, permissionSaved)
            return SettingsEffect.None
        }
        override fun dispose() { disposed = true }
    }

    @Test fun forwards_only_native_identities_and_raw_field_values() = runBlocking {
        val core = Fixture()
        val mounted = mutableListOf<ByteArray>()
        val presenter = SettingsPresenter(core, this, mounted::add, {}, {})
        presenter.refresh()
        yield()
        presenter.activate("settings-instance", 12uL, "confirm-credit")
        yield()
        presenter.change("settings-instance", 13uL, "device-name", "  🦀 Work  ")
        yield()
        assertEquals(listOf(Triple("settings-instance", 12uL, "confirm-credit")), core.events)
        assertEquals(listOf("  🦀 Work  "), core.fields)
        assertTrue(core.acknowledgments.isEmpty())
        assertTrue(mounted.isNotEmpty())
    }

    @Test fun mount_acknowledgment_belongs_to_the_native_adapter_not_frame_receipt() = runBlocking {
        val core = Fixture()
        var mounted = false
        val presenter = SettingsPresenter(core, this, {
            assertTrue(core.acknowledgments.isEmpty())
            mounted = true
            core.acknowledge("mounted-instance", 1uL)
        }, {}, {})
        presenter.refresh()
        yield()
        assertTrue(mounted)
        assertEquals(listOf("mounted-instance" to 1uL), core.acknowledgments)
    }

    @Test fun account_teardown_drops_a_late_effect_and_prevents_new_actions() = runBlocking {
        val response = CompletableDeferred<SettingsEffect>()
        val core = object : Fixture() {
            override suspend fun activate(instance: String, revision: ULong, key: String): SettingsEffect =
                withContext(NonCancellable) { response.await() }
        }
        val effects = mutableListOf<SettingsEffect>()
        val presenter = SettingsPresenter(core, this, {}, effects::add, {})
        presenter.activate("old-account", 1uL, "share-app")
        yield()
        presenter.dispose()
        response.complete(SettingsEffect.ShareText("Test share", "https://example.invalid/test"))
        yield()
        presenter.activate("new-account", 2uL, "delete")
        presenter.refresh()
        yield()
        assertTrue(core.disposed)
        assertTrue(effects.isEmpty())
        assertEquals(0, core.reads)
    }

    @Test fun a_late_snapshot_cannot_replace_a_newer_frame() = runBlocking {
        val response = CompletableDeferred<ByteArray>()
        val core = object : Fixture() {
            override suspend fun snapshot(): ByteArray {
                reads++
                return if (reads == 1) withContext(NonCancellable) { response.await() } else byteArrayOf(2)
            }
        }
        val mounted = mutableListOf<ByteArray>()
        val presenter = SettingsPresenter(core, this, mounted::add, {}, {})
        presenter.refresh(); yield()
        presenter.refresh(); yield()
        response.complete(byteArrayOf(1)); yield()
        assertEquals(1, mounted.size)
        assertArrayEquals(byteArrayOf(2), mounted.single())
    }

    @Test fun wallpaper_result_keeps_the_rust_ticket_and_permission_result() = runBlocking {
        val core = Fixture()
        val presenter = SettingsPresenter(core, this, {}, {}, {})
        presenter.wallpaper("rust-ticket", "content://fixture/image", "Picture", false)
        yield()
        assertEquals(listOf(listOf("rust-ticket", "content://fixture/image", "Picture", false)), core.wallpapers)
    }
}
