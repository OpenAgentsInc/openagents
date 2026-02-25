<?php

namespace WorkOS\Session;

/**
 * Interface SessionEncryptionInterface
 *
 * Defines the contract for session encryption/decryption providers.
 */
interface SessionEncryptionInterface
{
    /**
     * Encrypts and seals session data.
     *
     * @param array $data Session data to seal
     * @param string $password Encryption password
     * @param int|null $ttl Time-to-live in seconds (null for default)
     *
     * @return string Sealed (encrypted) session string
     * @throws \WorkOS\Exception\UnexpectedValueException
     */
    public function seal(array $data, string $password, ?int $ttl = null): string;

    /**
     * Decrypts and unseals session data.
     *
     * @param string $sealed Sealed session string
     * @param string $password Decryption password
     *
     * @return array Unsealed session data
     * @throws \WorkOS\Exception\UnexpectedValueException
     */
    public function unseal(string $sealed, string $password): array;
}
