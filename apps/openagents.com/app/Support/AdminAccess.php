<?php

namespace App\Support;

class AdminAccess
{
    /**
     * @return array<int, string>
     */
    public static function emails(): array
    {
        $configured = config('admin.emails', []);

        if (! is_array($configured)) {
            return [];
        }

        $emails = array_map(
            static fn (mixed $email): string => strtolower(trim((string) $email)),
            $configured,
        );

        $emails = array_values(array_filter(
            $emails,
            static fn (string $email): bool => $email !== '',
        ));

        return array_values(array_unique($emails));
    }

    public static function isAdminEmail(?string $email): bool
    {
        if (! is_string($email) || trim($email) === '') {
            return false;
        }

        return in_array(strtolower(trim($email)), self::emails(), true);
    }
}
