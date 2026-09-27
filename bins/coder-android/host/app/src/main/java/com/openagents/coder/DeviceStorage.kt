package com.openagents.coder

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileNotFoundException
import java.nio.file.Files
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Device-only encrypted native storage. Rust independently encrypts transcript caches. */
class DeviceStorage(private val context: Context, private val synthetic: Boolean, doorScope: String? = null) {
    private val scope = if (synthetic) "synthetic-v1" else "device-v1"
    private val doorName = "door-preferences" + if (synthetic && doorScope != null &&
        doorScope.matches(Regex("[A-Za-z0-9-]{1,64}"))) "-$doorScope" else ""

    fun identity(purpose: String): String = synchronized(storageLock) {
        require(purpose.matches(Regex("[a-z][a-z0-9-]{0,63}"))) { "Invalid device identity purpose." }
        val name = "$purpose-identity"
        val bytes = read(name) ?: ByteArray(32).also {
            SecureRandom().nextBytes(it)
            write(name, it)
        }
        check(bytes.size == 32) { "The device identity is unavailable. Reopen Coder after unlocking the device." }
        bytes.joinToString("") { "%02x".format(it.toInt() and 255) }
    }

    fun cacheDirectory(): File = synchronized(storageLock) {
        File(context.noBackupFilesDir, "$scope-reader").also {
            check(it.isDirectory || it.mkdirs() || it.isDirectory) { "The protected chat cache could not be opened." }
        }
    }

    fun gymCode(): String? = synchronized(storageLock) { read("gym-grant")?.toString(Charsets.UTF_8) }
    fun saveGymCode(code: String) = synchronized(storageLock) {
        val bytes = code.toByteArray(Charsets.UTF_8)
        require(bytes.size <= 65_536) { "The Gym connection exceeds its size limit." }
        write("gym-grant", bytes)
    }

    fun worldRelay(): String? = synchronized(storageLock) {
        val bytes = read("world-relay")
        check(bytes == null || bytes.size <= 2048) { "The saved relay URL is too long." }
        bytes?.takeIf { it.isNotEmpty() }?.toString(Charsets.UTF_8)
    }

    fun saveWorldRelay(relay: String?) = synchronized(storageLock) {
        val bytes = (relay ?: "").toByteArray(Charsets.UTF_8)
        require(bytes.size <= 2048) { "The relay URL is too long." }
        // An encrypted empty value is an atomic tombstone for explicit Leave.
        write("world-relay", bytes)
    }

    fun doorPreferences(): String? = synchronized(storageLock) {
        val bytes = read(doorName)
        check(bytes == null || bytes.size <= 2048) { "Saved door choices unavailable. Unlock the device and retry." }
        bytes?.toString(Charsets.UTF_8)
    }

    fun saveDoorPreferences(document: String) = synchronized(storageLock) {
        val bytes = document.toByteArray(Charsets.UTF_8)
        require(bytes.size <= 2048) { "Door choice not saved." }
        write(doorName, bytes)
    }

    private fun key(name: String): SecretKey {
        val alias = "com.openagents.coder.$scope.$name"
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(alias, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true).build())
            generateKey()
        }
    }

    private fun file(name: String) = AtomicFile(File(context.noBackupFilesDir, "$scope-$name.bin"))
    private fun read(name: String): ByteArray? {
        val file = file(name)
        // openRead restores an interrupted atomic write before inspecting its
        // contents. Checking the base file first can discard a recoverable key.
        val stream = try { file.openRead() }
        catch (failure: FileNotFoundException) {
            val backup = File(file.baseFile.path + ".bak")
            if (Files.notExists(file.baseFile.toPath()) && Files.notExists(backup.toPath())) return null
            throw failure
        }
        val bytes = stream.use { source ->
            val output = ByteArrayOutputStream()
            val chunk = ByteArray(4096)
            while (true) {
                val read = source.read(chunk)
                if (read < 0) break
                check(output.size() + read <= 65_600) { "Protected device storage has an invalid length." }
                output.write(chunk, 0, read)
            }
            output.toByteArray()
        }
        check(bytes.size in 29..65_600) { "Protected device storage has an invalid length." }
        check(bytes[0] == 1.toByte()) { "Protected device storage has an unsupported version." }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(name), GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
        cipher.updateAAD("$scope:$name".toByteArray())
        return cipher.doFinal(bytes.copyOfRange(13, bytes.size))
    }

    private fun write(name: String, data: ByteArray) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key(name))
        cipher.updateAAD("$scope:$name".toByteArray())
        val encoded = byteArrayOf(1) + cipher.iv + cipher.doFinal(data)
        val file = file(name)
        val stream = file.startWrite()
        try { stream.write(encoded); file.finishWrite(stream) }
        catch (failure: Exception) { file.failWrite(stream); throw failure }
    }

    companion object {
        // Activity recreation can overlap the old reader's final worker call
        // with a new host. Protect first creation across storage instances.
        private val storageLock = Any()
    }
}
