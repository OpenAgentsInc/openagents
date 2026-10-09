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
 * This device's secrets, each encrypted with its own Android Keystore key
 * that never leaves the device's secure hardware, in app-private storage
 * that is not backed up. No secret is printed or put in shared preferences.
 */
object DeviceKey {
    /**
     * A secret's purpose: `device` holds host grants; `world` signs only
     * Verse presence; `spark` is the phone wallet's BIP39 entropy; `gym` is
     * the saved Gym connection code for the world key; `iroh` is the key the
     * phone's iroh endpoint proves when it connects a computer, kept beside
     * the device key and never authorizing anything. Each has its own
     * Keystore key and file.
     */
    enum class Purpose(val id: String, val maxBytes: Int) {
        DEVICE("device", 32), WORLD("world", 32), SPARK("spark", 32), GYM("gym", 65_536), IROH("iroh", 32),
        // The person's own model provider keys (BYOK, #10176), one each.
        PROVIDER_OPENROUTER("provider-openrouter", 512), PROVIDER_VERCEL("provider-vercel", 512),
        PROVIDER_TYPESAFE("provider-typesafe", 512),
        // The openagents.com account session (#11107), as Rust's JSON.
        ACCOUNT("account", 8192),
    }
    private val lock = Any()

    /** The 32-byte secret key as lowercase hex, created on first use. */
    fun loadOrCreate(context: Context, purpose: Purpose = Purpose.DEVICE): String = synchronized(lock) {
        require(purpose == Purpose.DEVICE || purpose == Purpose.WORLD || purpose == Purpose.IROH)
        val secret = try { read(context, purpose) ?: random(32).also { write(context, purpose, it) } }
        catch (failure: Exception) {
            throw IllegalStateException("The device key is unavailable. Unlock the device and reopen OpenAgents.", failure)
        }
        check(secret.size == 32) { "The device key is unavailable. Unlock the device and reopen OpenAgents." }
        hex(secret)
    }

    /**
     * The Spark wallet's seed: 16 bytes of BIP39 entropy made on this phone,
     * or the 16 or 32 bytes a restore saved, as lowercase hex.
     */
    fun loadOrCreateSpark(context: Context): String = synchronized(lock) {
        val seed = try { read(context, Purpose.SPARK) ?: random(16).also { write(context, Purpose.SPARK, it) } }
        catch (failure: Exception) {
            throw IllegalStateException("The wallet's key is unavailable. Unlock the device and try again.", failure)
        }
        check(seed.size == 16 || seed.size == 32) { "The wallet's key is damaged." }
        hex(seed)
    }

    /** Replaces the Spark seed after Rust checked a restore's words. */
    fun replaceSpark(context: Context, entropyHex: String) = synchronized(lock) {
        val bytes = unhex(entropyHex)
        require(bytes != null && (bytes.size == 16 || bytes.size == 32)) { "The restored key is invalid." }
        try { write(context, Purpose.SPARK, bytes) }
        catch (failure: Exception) { throw IllegalStateException("The restored key could not be saved.", failure) }
    }

    /** The saved Gym connection code, if any. */
    fun gymCode(context: Context): String? = synchronized(lock) {
        try { read(context, Purpose.GYM)?.toString(Charsets.UTF_8) }
        catch (failure: Exception) {
            throw IllegalStateException("The saved Gym connection is unavailable. Unlock the device and try again.", failure)
        }
    }

    fun saveGymCode(context: Context, code: String) = synchronized(lock) {
        val bytes = code.toByteArray(Charsets.UTF_8)
        require(bytes.size <= Purpose.GYM.maxBytes) { "That Gym connection code is too long." }
        write(context, Purpose.GYM, bytes)
    }

    /** A provider key's purpose, for `openrouter`, `vercel`, or `typesafe`. */
    private fun providerPurpose(provider: String) = when (provider) {
        "openrouter" -> Purpose.PROVIDER_OPENROUTER
        "vercel" -> Purpose.PROVIDER_VERCEL
        "typesafe" -> Purpose.PROVIDER_TYPESAFE
        else -> null
    }

    /**
     * The person's own model provider keys (BYOK), as Rust's `provider_keys`
     * request takes them: read only to hand to Rust at start. A key that
     * can't be read is left out.
     */
    fun providerKeys(context: Context): org.json.JSONArray = synchronized(lock) {
        val keys = org.json.JSONArray()
        for (provider in listOf("openrouter", "vercel", "typesafe")) {
            val purpose = providerPurpose(provider) ?: continue
            val key = runCatching { read(context, purpose) }.getOrNull()?.toString(Charsets.UTF_8) ?: continue
            keys.put(org.json.JSONObject().put("provider", provider).put("key", key))
        }
        keys
    }

    /** Keeps a provider key once Rust's test accepted it. */
    fun saveProviderKey(context: Context, provider: String, key: String): Boolean = synchronized(lock) {
        val purpose = providerPurpose(provider) ?: return false
        val bytes = key.toByteArray(Charsets.UTF_8)
        if (bytes.isEmpty() || bytes.size > purpose.maxBytes) return false
        runCatching { write(context, purpose, bytes) }.isSuccess
    }

    /** Deletes a provider key and its Keystore key. */
    fun deleteProviderKey(context: Context, provider: String): Unit = synchronized(lock) {
        val purpose = providerPurpose(provider) ?: return
        file(context, purpose).delete()
        runCatching { KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(alias(purpose)) }
    }

    /** The openagents.com account session Rust handed over, as its JSON; null without one. */
    fun loadAccountSession(context: Context): String? = synchronized(lock) {
        runCatching { read(context, Purpose.ACCOUNT) }.getOrNull()?.toString(Charsets.UTF_8)
    }

    /** Keeps the account session Rust handed over. */
    fun saveAccountSession(context: Context, session: String): Boolean = synchronized(lock) {
        val bytes = session.toByteArray(Charsets.UTF_8)
        if (bytes.isEmpty() || bytes.size > Purpose.ACCOUNT.maxBytes) return false
        runCatching { write(context, Purpose.ACCOUNT, bytes) }.isSuccess
    }

    /** Forgets the account session and its Keystore key. */
    fun deleteAccountSession(context: Context): Unit = synchronized(lock) {
        file(context, Purpose.ACCOUNT).delete()
        runCatching { KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(alias(Purpose.ACCOUNT)) }
    }

    /** The app's private state directory, excluded from backup. */
    fun stateDirectory(context: Context): File =
        File(context.noBackupFilesDir, "openagents-v1").also {
            check(it.isDirectory || it.mkdirs() || it.isDirectory) { "The app's private storage could not be opened." }
        }

    internal fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it.toInt() and 255) }

    internal fun unhex(text: String): ByteArray? {
        if (text.length % 2 != 0 || text.any { Character.digit(it, 16) < 0 }) return null
        return ByteArray(text.length / 2) { ((Character.digit(text[it * 2], 16) shl 4) + Character.digit(text[it * 2 + 1], 16)).toByte() }
    }

    private fun random(size: Int) = ByteArray(size).also { SecureRandom().nextBytes(it) }

    private fun alias(purpose: Purpose) = "com.openagents.app.${purpose.id}-v1"

    private fun key(purpose: Purpose): SecretKey {
        val alias = alias(purpose)
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

    private fun file(context: Context, purpose: Purpose) =
        AtomicFile(File(context.noBackupFilesDir, "${purpose.id}-v1-key.bin"))

    private fun read(context: Context, purpose: Purpose): ByteArray? {
        val file = file(context, purpose)
        // openRead restores an interrupted atomic write before reading.
        val bytes = try { file.openRead().use { it.readBytes() } }
        catch (failure: FileNotFoundException) {
            if (!file.baseFile.exists() && !File(file.baseFile.path + ".bak").exists()) return null
            throw failure
        }
        // A version byte, a 12-byte nonce, the ciphertext, and a 16-byte tag.
        check(bytes.size in 29..(29 + purpose.maxBytes) && bytes[0] == 1.toByte()) { "The ${purpose.id} key record is damaged." }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(purpose), GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
        cipher.updateAAD(alias(purpose).toByteArray())
        return cipher.doFinal(bytes.copyOfRange(13, bytes.size))
    }

    private fun write(context: Context, purpose: Purpose, secret: ByteArray) {
        require(secret.size in 1..purpose.maxBytes)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key(purpose))
        cipher.updateAAD(alias(purpose).toByteArray())
        val encoded = byteArrayOf(1) + cipher.iv + cipher.doFinal(secret)
        val file = file(context, purpose)
        val stream = file.startWrite()
        try { stream.write(encoded); file.finishWrite(stream) }
        catch (failure: Exception) { file.failWrite(stream); throw failure }
    }
}
