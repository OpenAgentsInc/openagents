<?php

namespace App\Lightning\L402;

use App\Models\L402Credential;

final class L402CredentialCache
{
    public function get(string $host, string $scope): ?L402CredentialValue
    {
        /** @var L402Credential|null $row */
        $row = L402Credential::query()
            ->where('host', $host)
            ->where('scope', $scope)
            ->first();

        if (! $row) {
            return null;
        }

        $expiresAt = $row->expires_at;
        if (! $expiresAt || $expiresAt->isPast()) {
            $row->delete();

            return null;
        }

        return new L402CredentialValue(
            macaroon: (string) $row->macaroon,
            preimage: (string) $row->preimage,
            expiresAt: $expiresAt,
        );
    }

    public function put(string $host, string $scope, string $macaroon, string $preimage, int $ttlSeconds): void
    {
        $expiresAt = now()->addSeconds(max(1, $ttlSeconds));

        L402Credential::query()->updateOrCreate(
            ['host' => $host, 'scope' => $scope],
            [
                'macaroon' => $macaroon,
                'preimage' => $preimage,
                'expires_at' => $expiresAt,
            ],
        );
    }

    public function delete(string $host, string $scope): void
    {
        L402Credential::query()->where('host', $host)->where('scope', $scope)->delete();
    }
}
