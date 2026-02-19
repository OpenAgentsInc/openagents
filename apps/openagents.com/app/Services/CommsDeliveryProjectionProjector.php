<?php

namespace App\Services;

use App\Models\CommsDeliveryProjection;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use Carbon\Carbon;

class CommsDeliveryProjectionProjector
{
    /**
     * @param  array<string, mixed>  $payload
     */
    public function projectFromRuntimeDelivery(array $payload, int $webhookEventId): ?CommsDeliveryProjection
    {
        $userId = $payload['user_id'] ?? null;

        if (! is_int($userId) || $userId <= 0) {
            return null;
        }

        $provider = is_string($payload['provider'] ?? null) ? (string) $payload['provider'] : 'resend';
        $integrationId = is_string($payload['integration_id'] ?? null) && trim((string) $payload['integration_id']) !== ''
            ? (string) $payload['integration_id']
            : 'unknown';

        $lastEventAt = null;
        if (is_string($payload['occurred_at'] ?? null)) {
            try {
                $lastEventAt = Carbon::parse((string) $payload['occurred_at']);
            } catch (\Throwable) {
                $lastEventAt = now();
            }
        }

        $projection = CommsDeliveryProjection::query()->updateOrCreate(
            [
                'user_id' => $userId,
                'provider' => $provider,
                'integration_id' => $integrationId,
            ],
            [
                'last_state' => is_string($payload['delivery_state'] ?? null) ? (string) $payload['delivery_state'] : null,
                'last_event_at' => $lastEventAt,
                'last_message_id' => is_string($payload['message_id'] ?? null) ? (string) $payload['message_id'] : null,
                'last_recipient' => is_string($payload['recipient'] ?? null) ? (string) $payload['recipient'] : null,
                'runtime_event_id' => is_string($payload['event_id'] ?? null) ? (string) $payload['event_id'] : null,
                'source' => 'runtime_forwarder',
                'last_webhook_event_id' => $webhookEventId,
            ],
        );

        $integration = UserIntegration::query()
            ->where('user_id', $userId)
            ->where('provider', $provider)
            ->first();

        if ($integration) {
            UserIntegrationAudit::query()->create([
                'user_id' => $userId,
                'user_integration_id' => $integration->id,
                'provider' => $provider,
                'action' => 'delivery_projection_updated',
                'metadata' => [
                    'projection_id' => $projection->id,
                    'delivery_state' => $projection->last_state,
                    'message_id' => $projection->last_message_id,
                    'source' => 'runtime_forwarder',
                ],
            ]);
        }

        return $projection;
    }
}
