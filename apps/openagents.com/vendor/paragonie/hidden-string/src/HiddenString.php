<?php
declare(strict_types=1);
namespace ParagonIE\HiddenString;

use ParagonIE\ConstantTime\Binary;
use Throwable;
use TypeError;
use function
    hash_equals,
    sodium_memzero,
    str_repeat;

/**
 * Class HiddenString
 *
 * The purpose of this class is to encapsulate strings and hide their contents
 * from stack traces should an unhandled exception occur.
 *
 * The only things that should be protected:
 * - Passwords
 * - Plaintext (before encryption)
 * - Plaintext (after decryption)
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */
final class HiddenString
{
    protected string $internalStringValue = '';

    /**
     * Disallow the contents from being accessed via __toString()?
     */
    protected bool $disallowInline = true;

    /**
     * Disallow the contents from being accessed via __sleep()?
     */
    protected bool $disallowSerialization = true;

    /**
     * HiddenString constructor.
     *
     * @param string $value
     *
     * @param bool $disallowInline
     * @param bool $disallowSerialization
     *
     * @throws TypeError
     */
    public function __construct(
        #[\SensitiveParameter]
        string $value,
        bool $disallowInline = true,
        bool $disallowSerialization = true
    ) {
        $this->internalStringValue = self::safeStrcpy($value);
        $this->disallowInline = $disallowInline;
        $this->disallowSerialization = $disallowSerialization;
    }

    /**
     * @param HiddenString $other
     *
     * @return bool
     *
     * @throws TypeError
     */
    public function equals(HiddenString $other)
    {
        return hash_equals(
            $this->getString(),
            $other->getString()
        );
    }

    /**
     * Hide its internal state from var_dump()
     *
     * Note: The xdebug extension may break this behavior.
     * You should not rely on it if you have debugging extensions installed.
     *
     * @return array
     */
    public function __debugInfo()
    {
        return [
            'internalStringValue' =>
                '*',
            'attention' =>
                'If you need the value of a HiddenString, ' .
                'invoke getString() instead of dumping it.'
        ];
    }

    /**
     * Wipe it from memory after it's been used.
     *
     * @return void
     */
    public function __destruct()
    {
        if (is_callable('sodium_memzero')) {
            try {
                sodium_memzero($this->internalStringValue);
                return;
            } catch (Throwable $ex) {
            }
        }
        if (is_null($this->internalStringValue)) {
            return;
        }

        // Last-ditch attempt to wipe existing values if libsodium is not
        // available. Don't rely on this.
        $zero = str_repeat("\0", Binary::safeStrlen($this->internalStringValue));
        $this->internalStringValue = $this->internalStringValue ^ (
            $zero ^ $this->internalStringValue
        );
        unset($zero);
        unset($this->internalStringValue);
    }

    /**
     * Explicit invocation -- get the raw string value
     *
     * @return string
     *
     * @throws TypeError
     */
    public function getString(): string
    {
        return self::safeStrcpy($this->internalStringValue);
    }

    /**
     * Returns a copy of the string's internal value, which should be zeroed.
     * Optionally, it can return an empty string.
     *
     * @return string
     *
     * @throws MisuseException
     * @throws TypeError
     */
    public function __toString(): string
    {
        if (!$this->disallowInline) {
            return self::safeStrcpy($this->internalStringValue);
        }
        throw new MisuseException(
            'This HiddenString object cannot be inlined as a string.'
        );
    }

    /**
     * @return array
     *
     * @throws MisuseException
     */
    public function __sleep(): array
    {
        if (!$this->disallowSerialization) {
            return [
                'internalStringValue',
                'disallowInline',
                'disallowSerialization'
            ];
        }
        throw new MisuseException(
            'This HiddenString object cannot be serialized.'
        );
    }

    /**
     * PHP 7 uses interned strings. We don't want altering this one to alter
     * the original string.
     *
     * @param string $string
     *
     * @return string
     *
     * @throws TypeError
     */
    public static function safeStrcpy(
        #[\SensitiveParameter]
        string $string
    ): string {
        $length = Binary::safeStrlen($string);
        $return = '';
        $chunk = $length >> 1;
        if ($chunk < 1) {
            $chunk = 1;
        }
        for ($i = 0; $i < $length; $i += $chunk) {
            $return .= Binary::safeSubstr($string, $i, $chunk);
        }
        return $return;
    }
}
