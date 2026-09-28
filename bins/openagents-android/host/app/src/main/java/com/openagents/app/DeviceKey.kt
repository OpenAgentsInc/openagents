package com.openagents.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.io.File
import java.io.FileNotFoundException
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * This device's Nostr key, encrypted with an Android Keystore key that never
 * leaves the device's secure hardware, in app-private storage that is not
 * backed up. No secret is printed or put in shared preferences.
 */
object DeviceKey {
    private const val ALIAS = "com.openagents.app.device-v1"
    private const val NAME = "device-v1-key.bin"
    private val lock = Any()

    /** The 32-byte secret key as lowercase hex, created on first use. */
    fun loadOrCreate(context: Context): String = synchronized(lock) {
        val secret = try { read(context) ?: ByteArray(32).also { SecureRandom().nextBytes(it); write(context, it) } }
        catch (failure: Exception) {
            throw IllegalStateException("The device key is unavailable. Unlock the device and reopen OpenAgents.", failure)
        }
        check(secret.size == 32) { "The device key is unavailable. Unlock the device and reopen OpenAgents." }
        secret.joinToString("") { "%02x".format(it.toInt() and 255) }
    }

    /** The app's private state directory, excluded from backup. */
    fun stateDirectory(context: Context): File =
        File(context.noBackupFilesDir, "openagents-v1").also {
            check(it.isDirectory || it.mkdirs() || it.isDirectory) { "The app's private storage could not be opened." }
        }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true).build())
            generateKey()
        }
    }

    private fun file(context: Context) = AtomicFile(File(context.noBackupFilesDir, NAME))

    private fun read(context: Context): ByteArray? {
        val file = file(context)
        // openRead restores an interrupted atomic write before reading.
        val bytes = try { file.openRead().use { it.readBytes() } }
        catch (failure: FileNotFoundException) {
            if (!file.baseFile.exists() && !File(file.baseFile.path + ".bak").exists()) return null
            throw failure
        }
        check(bytes.size in 29..256 && bytes[0] == 1.toByte()) { "The device key record is damaged." }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
        cipher.updateAAD(ALIAS.toByteArray())
        return cipher.doFinal(bytes.copyOfRange(13, bytes.size))
    }

    private fun write(context: Context, secret: ByteArray) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(ALIAS.toByteArray())
        val encoded = byteArrayOf(1) + cipher.iv + cipher.doFinal(secret)
        val file = file(context)
        val stream = file.startWrite()
        try { stream.write(encoded); file.finishWrite(stream) }
        catch (failure: Exception) { file.failWrite(stream); throw failure }
    }
}
