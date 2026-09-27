package com.openagents.coder

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.security.KeyStore
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Every fixture uses an isolated test identity; saved user credentials are untouched. */
@RunWith(AndroidJUnit4::class)
class DeviceStorageTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val purposes = mutableListOf<String>()

    private fun purpose() = "acceptance-${UUID.randomUUID()}".also { purposes.add(it) }
    private fun file(purpose: String) = File(context.noBackupFilesDir, "synthetic-v1-$purpose-identity.bin")
    private fun storage() = DeviceStorage(context, true)

    @After fun cleanFixtures() {
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        for (purpose in purposes) {
            val file = file(purpose)
            listOf(file, File(file.path + ".bak"), File(file.path + ".new")).forEach { it.delete() }
            keys.deleteEntry("com.openagents.coder.synthetic-v1.$purpose-identity")
            val normal = File(context.noBackupFilesDir, "device-v1-$purpose-identity.bin")
            listOf(normal, File(normal.path + ".bak"), File(normal.path + ".new")).forEach { it.delete() }
            keys.deleteEntry("com.openagents.coder.device-v1.$purpose-identity")
        }
    }

    @Test fun syntheticAndNormalScopesNeverShareIdentities() {
        val purpose = purpose()
        val synthetic = storage().identity(purpose)
        val normal = DeviceStorage(context, false).identity(purpose)
        assertNotEquals(synthetic, normal)
        assertEquals(synthetic, storage().identity(purpose))
        assertEquals(normal, DeviceStorage(context, false).identity(purpose))
    }

    @Test fun interruptedAtomicWriteRecoversTheSameIdentity() {
        val purpose = purpose()
        val identity = storage().identity(purpose)
        val base = file(purpose)
        assertTrue(base.renameTo(File(base.path + ".bak")))
        assertFalse(base.exists())
        assertEquals(identity, storage().identity(purpose))
        assertTrue(base.exists())
        assertFalse(File(base.path + ".bak").exists())
    }

    @Test fun interruptedWriteRecoversBeforeCheckingTheBaseLength() {
        val purpose = purpose()
        val identity = storage().identity(purpose)
        val base = file(purpose)
        assertTrue(base.renameTo(File(base.path + ".bak")))
        base.writeBytes(byteArrayOf(1))
        assertEquals(identity, storage().identity(purpose))
    }

    @Test fun simultaneousStorageInstancesUseOneIdentity() {
        val purpose = purpose()
        val ready = CountDownLatch(8)
        val start = CountDownLatch(1)
        val pool = Executors.newFixedThreadPool(8)
        try {
            val results = (0 until 8).map {
                pool.submit<String> {
                    val storage = storage()
                    ready.countDown()
                    check(start.await(10, TimeUnit.SECONDS))
                    storage.identity(purpose)
                }
            }
            assertTrue(ready.await(10, TimeUnit.SECONDS))
            start.countDown()
            val identities = results.map { it.get(15, TimeUnit.SECONDS) }
            assertEquals(1, identities.toSet().size)
            assertEquals(identities.first(), storage().identity(purpose))
        } finally { start.countDown(); pool.shutdownNow() }
    }

    @Test fun damagedOrOversizedCiphertextDoesNotReplaceTheIdentity() {
        val purpose = purpose()
        storage().identity(purpose)
        val base = file(purpose)
        val damaged = base.readBytes().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        base.writeBytes(damaged)
        assertThrows(Exception::class.java) { storage().identity(purpose) }
        assertArrayEquals(damaged, base.readBytes())
        val oversized = ByteArray(65_601) { 1 }
        base.writeBytes(oversized)
        assertThrows(IllegalStateException::class.java) { storage().identity(purpose) }
        assertEquals(65_601L, base.length())
    }
}
