<?php

namespace App\Services;

use App\Models\User;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use InvalidArgumentException;

class IntegrationSecretLifecycleService
{
    /**
     * @param  array<string, mixed>  $metadata
     * @return array{integration: UserIntegration, action: string}
     */
    public function upsertResend(User $user, string $apiKey, array $metadata = []): array
    {
        $fingerprint = hash('sha256', $apiKey);

        $integration = UserIntegration::query()->firstOrNew([
            'user_id' => $user->id,
            'provider' => 'resend',
        ]);

        $action = $this->resolveUpsertAction($integration, $fingerprint);
        $existingMetadata = is_array($integration->metadata) ? $integration->metadata : [];

        $integration->fill([
            'status' => 'active',
            'encrypted_secret' => $apiKey,
            'secret_fingerprint' => $fingerprint,
            'secret_last4' => substr($apiKey, -4),
            'metadata' => array_merge($existingMetadata, [
                'sender_email' => $metadata['sender_email'] ?? null,
                'sender_name' => $metadata['sender_name'] ?? null,
            ]),
            'connected_at' => now(),
            'disconnected_at' => null,
        ]);

        $integration->save();

        $this->audit($user, $integration, $action, [
            'status' => 'active',
            'sender_email' => $metadata['sender_email'] ?? null,
            'sender_name' => $metadata['sender_name'] ?? null,
            'secret_last4' => $integration->secret_last4,
        ]);

        return ['integration' => $integration, 'action' => $action];
    }

    /**
     * @param  array<string, mixed>  $tokenResponse
     * @return array{integration: UserIntegration, action: string}
     */
    public function upsertGoogle(User $user, array $tokenResponse): array
    {
        $integration = UserIntegration::query()->firstOrNew([
            'user_id' => $user->id,
            'provider' => 'google',
        ]);

        $existingMetadata = is_array($integration->metadata) ? $integration->metadata : [];
        $existingPayload = $this->decodeSecretPayload($integration->encrypted_secret);

        $refreshToken = $this->resolveString(
            $tokenResponse['refresh_token'] ?? null,
            $existingPayload['refresh_token'] ?? null,
        );

        if ($refreshToken === null || trim($refreshToken) === '') {
            throw new InvalidArgumentException('Google token response did not include refresh_token.');
        }

        $accessToken = $this->resolveString(
            $tokenResponse['access_token'] ?? null,
            $existingPayload['access_token'] ?? null,
        );
        $scope = $this->resolveString(
            $tokenResponse['scope'] ?? null,
            $existingPayload['scope'] ?? null,
        );
        $tokenType = $this->resolveString(
            $tokenResponse['token_type'] ?? null,
            $existingPayload['token_type'] ?? null,
        );
        $expiresAt = $this->resolveTokenExpiry(
            $tokenResponse['expires_in'] ?? null,
            $tokenResponse['expires_at'] ?? null,
            $existingPayload['expires_at'] ?? null,
        );

        $payload = [
            'provider' => 'google',
            'integration_id' => 'gmail.primary',
            'refresh_token' => $refreshToken,
            'access_token' => $accessToken,
            'scope' => $scope,
            'token_type' => $tokenType,
            'expires_at' => $expiresAt,
            'obtained_at' => now()->toISOString(),
        ];

        $secret = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        if (! is_string($secret) || trim($secret) === '') {
            throw new InvalidArgumentException('Unable to encode Google secret payload.');
        }

        $fingerprint = hash('sha256', $refreshToken);
        $action = $this->resolveUpsertAction($integration, $fingerprint);

        $integration->fill([
            'status' => 'active',
            'encrypted_secret' => $secret,
            'secret_fingerprint' => $fingerprint,
            'secret_last4' => substr($refreshToken, -4),
            'metadata' => array_merge($existingMetadata, [
                'integration_id' => 'gmail.primary',
                'scope' => $scope,
                'token_type' => $tokenType,
                'expires_at' => $expiresAt,
            ]),
            'connected_at' => now(),
            'disconnected_at' => null,
        ]);

        $integration->save();

        $this->audit($user, $integration, $action, [
            'status' => 'active',
            'integration_id' => 'gmail.primary',
            'scope' => $scope,
            'token_type' => $tokenType,
            'expires_at' => $expiresAt,
            'secret_last4' => $integration->secret_last4,
        ]);

        return ['integration' => $integration, 'action' => $action];
    }

    public function revokeResend(User $user): ?UserIntegration
    {
        return $this->revokeProvider($user, 'resend');
    }

    public function revokeGoogle(User $user): ?UserIntegration
    {
        return $this->revokeProvider($user, 'google');
    }

    public function auditTestRequest(User $user, UserIntegration $integration): void
    {
        $this->audit($user, $integration, 'test_requested', [
            'status' => $integration->status,
            'secret_last4' => $integration->secret_last4,
        ]);
    }

    private function resolveUpsertAction(UserIntegration $integration, string $fingerprint): string
    {
        if (! $integration->exists || ! is_string($integration->secret_fingerprint) || trim($integration->secret_fingerprint) === '') {
            return 'secret_created';
        }

        if ($integration->secret_fingerprint !== $fingerprint) {
            return 'secret_rotated';
        }

        return 'secret_updated';
    }

    /**
     * @param  array<string, mixed>  $metadata
     */
    private function audit(User $user, UserIntegration $integration, string $action, array $metadata = []): void
    {
        UserIntegrationAudit::query()->create([
            'user_id' => $user->id,
            'user_integration_id' => $integration->id,
            'provider' => $integration->provider,
            'action' => $action,
            'metadata' => $metadata,
        ]);
    }

    private function revokeProvider(User $user, string $provider): ?UserIntegration
    {
        $integration = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', $provider)
            ->first();

        if (! $integration) {
            return null;
        }

        $integration->fill([
            'status' => 'inactive',
            'encrypted_secret' => null,
            'secret_fingerprint' => null,
            'secret_last4' => null,
            'disconnected_at' => now(),
        ]);

        $integration->save();

        $this->audit($user, $integration, 'secret_revoked', [
            'status' => 'inactive',
        ]);

        return $integration;
    }

    /**
     * @return array<string, mixed>
     */
    private function decodeSecretPayload(mixed $encryptedSecret): array
    {
        if (! is_string($encryptedSecret) || trim($encryptedSecret) === '') {
            return [];
        }

        $decoded = json_decode($encryptedSecret, true);

        return is_array($decoded) ? $decoded : [];
    }

    private function resolveString(mixed $primary, mixed $fallback = null): ?string
    {
        if (is_string($primary) && trim($primary) !== '') {
            return $primary;
        }

        if (is_string($fallback) && trim($fallback) !== '') {
            return $fallback;
        }

        return null;
    }

    private function resolveTokenExpiry(mixed $expiresIn, mixed $expiresAt, mixed $fallback): ?string
    {
        if (is_numeric($expiresIn)) {
            return now()->addSeconds((int) $expiresIn)->toISOString();
        }

        if (is_string($expiresAt) && trim($expiresAt) !== '') {
            return $expiresAt;
        }

        if (is_string($fallback) && trim($fallback) !== '') {
            return $fallback;
        }

        return null;
    }
}
