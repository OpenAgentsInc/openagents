<?php

namespace App\Services;

use App\Models\User;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;

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

    public function revokeResend(User $user): ?UserIntegration
    {
        $integration = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
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
}
