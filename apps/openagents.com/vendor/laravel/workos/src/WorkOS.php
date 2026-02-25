<?php

namespace Laravel\WorkOS;

use Firebase\JWT\JWK;
use Firebase\JWT\JWT;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use RuntimeException;
use Throwable;
use WorkOS\UserManagement;
use WorkOS\WorkOS as SDK;

class WorkOS
{
    /**
     * Ensure WorkOS is configured.
     */
    public static function configure(): void
    {
        if (! config('services.workos.client_id')) {
            throw new RuntimeException("The 'services.workos.client_id' configuration value is undefined.");
        }

        if (! config('services.workos.secret')) {
            throw new RuntimeException("The 'services.workos.secret' configuration value is undefined.");
        }

        if (! config('services.workos.redirect_url')) {
            throw new RuntimeException("The 'services.workos.redirect_url' configuration value is undefined.");
        }

        SDK::setClientId(config('services.workos.client_id'));
        SDK::setApiKey(config('services.workos.secret'));
    }

    /**
     * Ensure the given access token is valid, refreshing it if necessary.
     */
    public static function ensureAccessTokenIsValid(string $accessToken, string $refreshToken): array
    {
        static::configure();

        $workOsSession = static::decodeAccessToken($accessToken);

        if (! $workOsSession) {
            $result = (new UserManagement)->authenticateWithRefreshToken(
                config('services.workos.client_id'), $refreshToken
            );

            return [
                $result->access_token,
                $result->refresh_token,
            ];
        }

        return [
            $accessToken,
            $refreshToken,
        ];
    }

    /**
     * Decode the given WorkOS access token.
     */
    public static function decodeAccessToken(string $accessToken): array|bool
    {
        static::configure();

        try {
            return (array) JWT::decode($accessToken, JWK::parseKeySet(static::getJwk()));
        } catch (Throwable $e) {
            //
        }

        return false;
    }

    /**
     * Get the WorkOS JWK.
     */
    protected static function getJwk(): array
    {
        return Cache::remember('workos:jwk', now()->addHours(12), function () {
            return Http::get(
                (new UserManagement)->getJwksUrl(config('services.workos.client_id'))
            )->json();
        });
    }
}
