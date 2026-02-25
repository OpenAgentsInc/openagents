<?php

namespace WorkOS\Session;

use ParagonIE\Halite\KeyFactory;
use ParagonIE\Halite\Symmetric\Crypto as SymmetricCrypto;
use ParagonIE\Halite\Symmetric\EncryptionKey;
use ParagonIE\HiddenString\HiddenString;
use WorkOS\Exception\UnexpectedValueException;

/**
 * Class HaliteSessionEncryption
 *
 * Implements session encryption using Paragonie Halite library (libsodium).
 */
class HaliteSessionEncryption implements SessionEncryptionInterface
{
    /**
     * Default TTL for WorkOS sessions (30 days).
     * WorkOS sessions last longer than typical 14-day cookie sessions.
     */
    private const DEFAULT_TTL = 2592000; // 30 days in seconds

    /**
     * Encrypts and seals session data with TTL.
     *
     * @param array $data Session data to encrypt
     * @param string $password Encryption password
     * @param int|null $ttl Time-to-live in seconds (null for default)
     *
     * @return string Base64-encoded sealed session
     * @throws \WorkOS\Exception\UnexpectedValueException
     */
    public function seal(array $data, string $password, ?int $ttl = null): string
    {
        try {
            $ttl = $ttl ?? self::DEFAULT_TTL;
            $expiresAt = time() + $ttl;

            $payload = [
                'data' => $data,
                'expires_at' => $expiresAt
            ];

            $key = $this->deriveKey($password);
            $encrypted = SymmetricCrypto::encrypt(
                new HiddenString(json_encode($payload)),
                $key
            );

            return base64_encode($encrypted);
        } catch (\Exception $e) {
            throw new UnexpectedValueException(
                "Failed to seal session: " . $e->getMessage()
            );
        }
    }

    /**
     * Decrypts and unseals session data with TTL validation.
     *
     * @param string $sealed Sealed session string
     * @param string $password Decryption password
     *
     * @return array Unsealed session data
     * @throws \WorkOS\Exception\UnexpectedValueException
     */
    public function unseal(string $sealed, string $password): array
    {
        try {
            $key = $this->deriveKey($password);
            $encrypted = base64_decode($sealed);

            $decryptedHiddenString = SymmetricCrypto::decrypt($encrypted, $key);
            $decrypted = $decryptedHiddenString->getString();
            $payload = json_decode($decrypted, true);

            if (!isset($payload['expires_at']) || !isset($payload['data'])) {
                throw new UnexpectedValueException("Invalid session payload");
            }

            if (time() > $payload['expires_at']) {
                throw new UnexpectedValueException("Session has expired");
            }

            return $payload['data'];
        } catch (UnexpectedValueException $e) {
            // Re-throw our exceptions
            throw $e;
        } catch (\Exception $e) {
            throw new UnexpectedValueException(
                "Failed to unseal session: " . $e->getMessage()
            );
        }
    }

    /**
     * Derives an encryption key from password using HKDF.
     *
     * @param string $password Password to derive key from
     *
     * @return EncryptionKey Encryption key for Halite
     * @throws \WorkOS\Exception\UnexpectedValueException
     */
    private function deriveKey(string $password): EncryptionKey
    {
        try {
            // Use HKDF to derive a 32-byte key from the password
            // This ensures the password is properly formatted for Halite
            $keyMaterial = hash_hkdf('sha256', $password, 32);

            return new EncryptionKey(new HiddenString($keyMaterial));
        } catch (\Exception $e) {
            throw new UnexpectedValueException(
                "Failed to derive encryption key: " . $e->getMessage()
            );
        }
    }
}
