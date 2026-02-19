<?php

namespace App\Jobs;

use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use App\Support\Comms\RuntimeCommsDeliveryForwarder;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Facades\DB;
use RuntimeException;

class ForwardResendWebhookToRuntime implements ShouldQueue
{
    use Dispatchable;
    use InteractsWithQueue;
    use SerializesModels;

    public int $tries = 3;

    public function __construct(public readonly int $webhookEventId) {}

    public function handle(RuntimeCommsDeliveryForwarder $forwarder): void
    {
        $row = DB::table('comms_webhook_events')->where('id', $this->webhookEventId)->first();

        if (! $row) {
            return;
        }

        if (! (bool) $row->signature_valid || ! is_string($row->normalized_payload) || trim($row->normalized_payload) === '') {
            return;
        }

        $payload = json_decode((string) $row->normalized_payload, true);
        if (! is_array($payload)) {
            $this->markFailed($row->id, null, null, 'runtime_payload_decode_failed');
            throw new RuntimeException('runtime payload decode failed');
        }

        DB::table('comms_webhook_events')
            ->where('id', $row->id)
            ->update([
                'runtime_attempts' => ((int) $row->runtime_attempts) + 1,
                'status' => 'forwarding',
                'updated_at' => now(),
            ]);

        $result = $forwarder->forward($payload);

        if (! ($result['ok'] ?? false)) {
            $this->markFailed(
                (int) $row->id,
                isset($result['status']) ? (int) $result['status'] : null,
                $result['body'] ?? null,
                is_string($result['error'] ?? null) ? (string) $result['error'] : 'runtime_forward_failed',
            );

            throw new RuntimeException('runtime delivery forwarding failed');
        }

        DB::table('comms_webhook_events')
            ->where('id', $row->id)
            ->update([
                'status' => 'forwarded',
                'runtime_status_code' => isset($result['status']) ? (int) $result['status'] : null,
                'runtime_response' => $this->encodeJson($result['body'] ?? null),
                'forwarded_at' => now(),
                'last_error' => null,
                'updated_at' => now(),
            ]);

        $this->updateIntegrationProjection($payload);
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function updateIntegrationProjection(array $payload): void
    {
        $userId = $payload['user_id'] ?? null;

        if (! is_int($userId) || $userId <= 0) {
            return;
        }

        $integration = UserIntegration::query()
            ->where('user_id', $userId)
            ->where('provider', 'resend')
            ->first();

        if (! $integration) {
            return;
        }

        $metadata = is_array($integration->metadata) ? $integration->metadata : [];
        $metadata['delivery_projection'] = [
            'last_state' => is_string($payload['delivery_state'] ?? null) ? $payload['delivery_state'] : null,
            'last_event_at' => is_string($payload['occurred_at'] ?? null) ? $payload['occurred_at'] : now()->toISOString(),
            'last_message_id' => is_string($payload['message_id'] ?? null) ? $payload['message_id'] : null,
            'last_recipient' => is_string($payload['recipient'] ?? null) ? $payload['recipient'] : null,
        ];

        $integration->metadata = $metadata;
        $integration->save();

        UserIntegrationAudit::query()->create([
            'user_id' => $userId,
            'user_integration_id' => $integration->id,
            'provider' => 'resend',
            'action' => 'delivery_projection_updated',
            'metadata' => [
                'delivery_state' => $metadata['delivery_projection']['last_state'] ?? null,
                'message_id' => $metadata['delivery_projection']['last_message_id'] ?? null,
            ],
        ]);
    }

    private function markFailed(int $rowId, ?int $statusCode, mixed $body, string $error): void
    {
        DB::table('comms_webhook_events')
            ->where('id', $rowId)
            ->update([
                'status' => 'failed',
                'runtime_status_code' => $statusCode,
                'runtime_response' => $this->encodeJson($body),
                'last_error' => $error,
                'updated_at' => now(),
            ]);
    }

    private function encodeJson(mixed $value): ?string
    {
        if ($value === null) {
            return null;
        }

        $encoded = json_encode($value, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

        return is_string($encoded) ? $encoded : null;
    }
}
