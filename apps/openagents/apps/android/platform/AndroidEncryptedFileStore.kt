// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.UserNotAuthenticatedException
import android.system.Os
import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.security.KeyStore
import java.security.MessageDigest
import java.util.UUID
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.openagents_core.AndroidSecretStorage
import uniffi.openagents_core.SecretMutation
import uniffi.openagents_core.SecretRead

/** Encrypted byte storage only. Rust chooses namespaces, keys, and lifecycle. */
class AndroidEncryptedFileStore(context: Context) : AndroidSecretStorage {
    private val appContext = context.applicationContext
    private val directory = File(appContext.filesDir, "openagents-secure")
    override fun read(namespace: String, key: String): SecretRead = synchronized(PROCESS_LOCK) {
        var envelope: ByteArray? = null
        try {
            val target = fileFor(namespace, key)
            if (!target.isFile) return@synchronized SecretRead.Absent
            if (target.length() > MAX_VALUE_BYTES + MAX_ENVELOPE_BYTES) {
                return@synchronized SecretRead.Corrupt
            }
            val stored = target.readBytes()
            envelope = stored
            val plaintext = decrypt(namespace, key, stored)
            SecretRead.Present(plaintext)
        } catch (_: UserNotAuthenticatedException) {
            SecretRead.Locked
        } catch (_: AEADBadTagException) {
            SecretRead.Corrupt
        } catch (_: IllegalArgumentException) {
            SecretRead.Corrupt
        } catch (_: Exception) {
            SecretRead.Failed
        } finally {
            envelope?.fill(0)
        }
    }

    override fun create(namespace: String, key: String, bytes: ByteArray): SecretMutation =
        mutate(namespace, key, bytes, replace = false)

    override fun replace(namespace: String, key: String, bytes: ByteArray): SecretMutation =
        mutate(namespace, key, bytes, replace = true)

    override fun delete(namespace: String, key: String): SecretMutation = synchronized(PROCESS_LOCK) {
        try {
            val target = fileFor(namespace, key)
            withFileLock(target) {
                if (target.exists()) {
                    if (!target.delete()) return@withFileLock SecretMutation.FAILED
                    SecretMutation.STORED
                } else {
                    SecretMutation.ABSENT
                }
            }
        } catch (_: Exception) {
            SecretMutation.FAILED
        }
    }

    private fun mutate(
        namespace: String,
        key: String,
        bytes: ByteArray,
        replace: Boolean,
    ): SecretMutation = synchronized(PROCESS_LOCK) {
        try {
            val target = fileFor(namespace, key)
            withFileLock(target) {
                if (bytes.size > MAX_VALUE_BYTES) return@withFileLock SecretMutation.FAILED
                if (!replace && target.exists()) return@withFileLock SecretMutation.ALREADY_EXISTS
                val envelope = encrypt(namespace, key, bytes)
                val temporary = File(directory, "${target.name}.${UUID.randomUUID()}.tmp")
                try {
                    RandomAccessFile(temporary, "rw").use { file ->
                        file.write(envelope)
                        file.fd.sync()
                    }
                    Os.rename(temporary.absolutePath, target.absolutePath)
                    SecretMutation.STORED
                } finally {
                    envelope.fill(0)
                    temporary.delete()
                }
            }
        } catch (_: UserNotAuthenticatedException) {
            SecretMutation.LOCKED
        } catch (_: Exception) {
            SecretMutation.FAILED
        } finally {
            bytes.fill(0)
        }
    }

    private fun fileFor(namespace: String, key: String): File {
        require(namespace.isNotBlank() && key.isNotBlank())
        if (!directory.exists() && !directory.mkdirs()) throw IllegalStateException()
        val digest = MessageDigest.getInstance("SHA-256")
            .digest("$namespace\u0000$key".toByteArray(Charsets.UTF_8))
        val name = digest.joinToString("") { byte -> "%02x".format(byte.toInt() and 0xff) }
        return File(directory, "$name.bin")
    }

    private fun withFileLock(target: File, block: () -> SecretMutation): SecretMutation {
        val lockFile = File(directory, "${target.name}.lock")
        RandomAccessFile(lockFile, "rw").use { random ->
            random.channel.use { channel ->
                channel.lock().use { return block() }
            }
        }
    }

    private fun encrypt(namespace: String, key: String, plaintext: ByteArray): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, encryptionKey())
        cipher.updateAAD(aad(namespace, key))
        val nonce = cipher.iv
        val cleartext = plaintext.copyOf()
        return try {
            val encrypted = cipher.doFinal(cleartext)
            try {
                ByteBuffer.allocate(MAGIC.size + nonce.size + encrypted.size)
                    .put(MAGIC).put(nonce).put(encrypted).array()
            } finally {
                encrypted.fill(0)
                nonce.fill(0)
            }
        } finally {
            cleartext.fill(0)
        }
    }

    private fun decrypt(namespace: String, key: String, envelope: ByteArray): ByteArray {
        if (envelope.size < MAGIC.size + NONCE_BYTES + TAG_BYTES) throw IllegalArgumentException()
        val input = ByteBuffer.wrap(envelope)
        val magic = ByteArray(MAGIC.size).also(input::get)
        if (!magic.contentEquals(MAGIC)) throw IllegalArgumentException()
        val nonce = ByteArray(NONCE_BYTES).also(input::get)
        val ciphertext = ByteArray(input.remaining()).also(input::get)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, encryptionKey(), GCMParameterSpec(TAG_BITS, nonce))
        cipher.updateAAD(aad(namespace, key))
        return try {
            cipher.doFinal(ciphertext)
        } finally {
            ciphertext.fill(0)
            nonce.fill(0)
            magic.fill(0)
        }
    }

    private fun encryptionKey(): SecretKey {
        synchronized(KEY_LOCK) {
            val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
            (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
            val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
            generator.init(
                KeyGenParameterSpec.Builder(
                    KEY_ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setRandomizedEncryptionRequired(true)
                    .setUserAuthenticationRequired(false)
                    .build(),
            )
            try {
                return generator.generateKey()
            } catch (error: Exception) {
                val refreshed = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
                (refreshed.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
                throw error
            }
        }
    }

    private fun aad(namespace: String, key: String): ByteArray =
        "v1\u0000$namespace\u0000$key".toByteArray(Charsets.UTF_8)

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "com.openagents.app.secure-storage.v1"
        const val NONCE_BYTES = 12
        const val TAG_BITS = 128
        const val TAG_BYTES = TAG_BITS / 8
        const val MAX_VALUE_BYTES = 1_048_576
        const val MAX_ENVELOPE_BYTES = 4 + NONCE_BYTES + TAG_BYTES
        val KEY_LOCK = Any()
        val PROCESS_LOCK = Any()
        val MAGIC = byteArrayOf('O'.code.toByte(), 'A'.code.toByte(), 'S'.code.toByte(), 1)
    }
}
