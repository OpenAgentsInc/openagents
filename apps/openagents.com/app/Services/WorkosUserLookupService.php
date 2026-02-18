<?php

namespace App\Services;

use Laravel\WorkOS\WorkOS;
use WorkOS\Exception\WorkOSException;
use WorkOS\UserManagement;

class WorkosUserLookupService
{
    /**
     * @return array{email:string,name:?string,avatar:?string}|null
     */
    public function lookupByWorkosId(string $workosId): ?array
    {
        $candidate = trim($workosId);
        if ($candidate === '') {
            return null;
        }

        WorkOS::configure();

        try {
            $workosUser = (new UserManagement)->getUser($candidate);
        } catch (WorkOSException) {
            return null;
        }

        if (! is_object($workosUser)) {
            return null;
        }

        $email = $this->resolveString($workosUser, ['email']);
        if (! is_string($email) || trim($email) === '' || ! filter_var($email, FILTER_VALIDATE_EMAIL)) {
            return null;
        }

        $firstName = $this->resolveString($workosUser, ['firstName', 'first_name']);
        $lastName = $this->resolveString($workosUser, ['lastName', 'last_name']);
        $name = trim(implode(' ', array_filter([$firstName, $lastName])));

        return [
            'email' => strtolower(trim($email)),
            'name' => $name !== '' ? $name : null,
            'avatar' => $this->resolveString($workosUser, ['profilePictureUrl', 'profile_picture_url']),
        ];
    }

    private function resolveString(object $source, array $keys): ?string
    {
        foreach ($keys as $key) {
            try {
                if (! isset($source->{$key})) {
                    continue;
                }
            } catch (\Throwable) {
                continue;
            }

            $value = $source->{$key};
            if (is_string($value)) {
                $trimmed = trim($value);
                if ($trimmed !== '') {
                    return $trimmed;
                }
            }
        }

        return null;
    }
}
