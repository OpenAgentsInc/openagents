package com.openagents.app.storage

import android.content.Context
import android.security.keystore.UserNotAuthenticatedException
import android.util.AtomicFile
import uniffi.openagents_core.AndroidRetiredWalletSeed
import uniffi.openagents_core.RetiredSeedRead
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Reads the encrypted seed created by the retired Android app. */
class AndroidRetiredSparkSeed(context: Context) : AndroidRetiredWalletSeed {
    private val file = AtomicFile(File(context.noBackupFilesDir, LEGACY_SEED_FILE))

    override fun readRetiredSeed(): RetiredSeedRead {
        val seedFile = file.baseFile
        if (!seedFile.exists()) return RetiredSeedRead.Absent

        return try {
            val record = file.openRead().use { it.readBytes() }
            decrypt(record)
        } catch (_: UserNotAuthenticatedException) {
            RetiredSeedRead.Locked
        } catch (_: SecurityException) {
            RetiredSeedRead.Locked
        } catch (_: Exception) {
            RetiredSeedRead.Failed
        }
    }

    private fun decrypt(record: ByteArray): RetiredSeedRead {
        if (record.size != LEGACY_SHORT_RECORD_SIZE && record.size != LEGACY_LONG_RECORD_SIZE) {
            return RetiredSeedRead.Corrupt
        }
        if (record[0] != LEGACY_VERSION) return RetiredSeedRead.Corrupt

        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        val key = keyStore.getKey(LEGACY_KEY_ALIAS, null) as? SecretKey
            ?: return RetiredSeedRead.Failed
        val nonce = record.copyOfRange(1, 1 + NONCE_SIZE)
        val ciphertext = record.copyOfRange(1 + NONCE_SIZE, record.size)
        return try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, nonce))
            cipher.updateAAD(LEGACY_KEY_ALIAS.toByteArray(Charsets.UTF_8))
            val entropy = cipher.doFinal(ciphertext)
            if (entropy.size != SHORT_ENTROPY_SIZE && entropy.size != LONG_ENTROPY_SIZE) {
                entropy.fill(0)
                RetiredSeedRead.Corrupt
            } else {
                RetiredSeedRead.Present(entropy)
            }
        } catch (_: UserNotAuthenticatedException) {
            RetiredSeedRead.Locked
        } catch (_: SecurityException) {
            RetiredSeedRead.Locked
        } catch (_: Exception) {
            RetiredSeedRead.Corrupt
        } finally {
            nonce.fill(0)
            ciphertext.fill(0)
        }
    }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val LEGACY_KEY_ALIAS = "com.openagents.app.spark-v1"
        const val LEGACY_SEED_FILE = "spark-v1-key.bin"
        const val LEGACY_VERSION: Byte = 1
        const val NONCE_SIZE = 12
        const val TAG_BITS = 128
        const val SHORT_ENTROPY_SIZE = 16
        const val LONG_ENTROPY_SIZE = 32
        const val LEGACY_SHORT_RECORD_SIZE = 1 + NONCE_SIZE + SHORT_ENTROPY_SIZE + 16
        const val LEGACY_LONG_RECORD_SIZE = 1 + NONCE_SIZE + LONG_ENTROPY_SIZE + 16
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
