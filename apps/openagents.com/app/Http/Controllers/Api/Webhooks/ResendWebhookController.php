<?php

namespace App\Http\Controllers\Api\Webhooks;

use App\Http\Controllers\Controller;
use App\Jobs\ForwardResendWebhookToRuntime;
use App\Support\Comms\ResendWebhookNormalizer;
use App\Support\Comms\ResendWebhookSignatureVerifier;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class ResendWebhookController extends Controller
{
    public function store(
        Request $request,
        ResendWebhookSignatureVerifier $signatureVerifier,
        ResendWebhookNormalizer $normalizer,
    ): JsonResponse {
        $provider = 'resend';
        $rawBody = (string) $request->getContent();
        $svixId = (string) $request->header('svix-id', '');
        $svixTimestamp = (string) $request->header('svix-timestamp', '');
        $svixSignature = (string) $request->header('svix-signature', '');

        $decodedRaw = json_decode($rawBody, true);
        $rawPayload = is_array($decodedRaw) ? $decodedRaw : [];

        $idempotencyKey = $this->idempotencyKey($provider, $svixId, $rawBody);

        $signatureValid = $signatureVerifier->verify($rawBody, $svixId, $svixTimestamp, $svixSignature);

        if (! $signatureValid) {
            $existingInvalid = DB::table('comms_webhook_events')->where('idempotency_key', $idempotencyKey)->first();

            if ($existingInvalid) {
                $recordId = (int) $existingInvalid->id;

                DB::table('comms_webhook_events')->where('id', $recordId)->update([
                    'signature_valid' => false,
                    'status' => 'invalid_signature',
                    'raw_payload' => json_encode($rawPayload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
                    'last_error' => 'invalid_signature',
                    'updated_at' => now(),
                ]);
            } else {
                $recordId = DB::table('comms_webhook_events')->insertGetId([
                    'provider' => $provider,
                    'idempotency_key' => $idempotencyKey,
                    'external_event_id' => $svixId !== '' ? $svixId : null,
                    'signature_valid' => false,
                    'status' => 'invalid_signature',
                    'raw_payload' => json_encode($rawPayload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
                    'last_error' => 'invalid_signature',
                    'created_at' => now(),
                    'updated_at' => now(),
                ]);
            }

            return response()->json([
                'error' => [
                    'code' => 'invalid_signature',
                    'message' => 'invalid webhook signature',
                ],
                'audit' => [
                    'event_id' => $recordId,
                ],
            ], 401);
        }

        $normalized = $normalizer->normalize($rawPayload, $svixId !== '' ? $svixId : null);
        $normalizedHash = $normalized ? $this->hashPayload($normalized) : null;

        $existing = DB::table('comms_webhook_events')->where('idempotency_key', $idempotencyKey)->first();

        if ($existing) {
            if (! (bool) $existing->signature_valid) {
                $status = $normalized ? 'received' : 'ignored';

                DB::table('comms_webhook_events')->where('id', (int) $existing->id)->update([
                    'signature_valid' => true,
                    'status' => $status,
                    'event_type' => is_array($rawPayload) && is_string($rawPayload['type'] ?? null) ? (string) $rawPayload['type'] : null,
                    'delivery_state' => is_array($normalized) && is_string($normalized['delivery_state'] ?? null) ? (string) $normalized['delivery_state'] : null,
                    'message_id' => is_array($normalized) && is_string($normalized['message_id'] ?? null) ? (string) $normalized['message_id'] : null,
                    'integration_id' => is_array($normalized) && is_string($normalized['integration_id'] ?? null) ? (string) $normalized['integration_id'] : null,
                    'user_id' => is_array($normalized) && is_int($normalized['user_id'] ?? null) ? (int) $normalized['user_id'] : null,
                    'recipient' => is_array($normalized) && is_string($normalized['recipient'] ?? null) ? (string) $normalized['recipient'] : null,
                    'normalized_hash' => $normalizedHash,
                    'normalized_payload' => $this->encodeJson($normalized),
                    'raw_payload' => json_encode($rawPayload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
                    'last_error' => null,
                    'updated_at' => now(),
                ]);

                if ($this->shouldDispatch($status, $normalized)) {
                    ForwardResendWebhookToRuntime::dispatch((int) $existing->id);
                }

                return response()->json([
                    'data' => [
                        'event_id' => (int) $existing->id,
                        'status' => $status,
                        'idempotent_replay' => false,
                    ],
                ], 202);
            }

            if (
                is_string($existing->normalized_hash)
                && is_string($normalizedHash)
                && ! hash_equals($existing->normalized_hash, $normalizedHash)
            ) {
                return response()->json([
                    'error' => [
                        'code' => 'idempotency_conflict',
                        'message' => 'webhook idempotency key conflicts with different normalized payload',
                    ],
                ], 409);
            }

            if ($this->shouldDispatch((string) $existing->status, $normalized)) {
                ForwardResendWebhookToRuntime::dispatch((int) $existing->id);
            }

            return response()->json([
                'data' => [
                    'event_id' => (int) $existing->id,
                    'status' => (string) $existing->status,
                    'idempotent_replay' => true,
                ],
            ], 200);
        }

        $status = $normalized ? 'received' : 'ignored';

        $recordId = DB::table('comms_webhook_events')->insertGetId([
            'provider' => $provider,
            'idempotency_key' => $idempotencyKey,
            'external_event_id' => $svixId !== '' ? $svixId : null,
            'event_type' => is_array($rawPayload) && is_string($rawPayload['type'] ?? null) ? (string) $rawPayload['type'] : null,
            'delivery_state' => is_array($normalized) && is_string($normalized['delivery_state'] ?? null) ? (string) $normalized['delivery_state'] : null,
            'message_id' => is_array($normalized) && is_string($normalized['message_id'] ?? null) ? (string) $normalized['message_id'] : null,
            'integration_id' => is_array($normalized) && is_string($normalized['integration_id'] ?? null) ? (string) $normalized['integration_id'] : null,
            'user_id' => is_array($normalized) && is_int($normalized['user_id'] ?? null) ? (int) $normalized['user_id'] : null,
            'recipient' => is_array($normalized) && is_string($normalized['recipient'] ?? null) ? (string) $normalized['recipient'] : null,
            'signature_valid' => true,
            'status' => $status,
            'normalized_hash' => $normalizedHash,
            'normalized_payload' => $this->encodeJson($normalized),
            'raw_payload' => json_encode($rawPayload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'created_at' => now(),
            'updated_at' => now(),
        ]);

        if ($this->shouldDispatch($status, $normalized)) {
            ForwardResendWebhookToRuntime::dispatch($recordId);
        }

        return response()->json([
            'data' => [
                'event_id' => $recordId,
                'status' => $status,
                'idempotent_replay' => false,
            ],
        ], 202);
    }

    private function idempotencyKey(string $provider, string $externalEventId, string $rawBody): string
    {
        if ($externalEventId !== '') {
            return sprintf('%s:%s', $provider, $externalEventId);
        }

        return sprintf('%s:body:%s', $provider, hash('sha256', $rawBody));
    }

    /**
     * @param  array<string, mixed>|null  $payload
     */
    private function hashPayload(?array $payload): ?string
    {
        if (! is_array($payload)) {
            return null;
        }

        $encoded = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

        if (! is_string($encoded)) {
            return null;
        }

        return hash('sha256', $encoded);
    }

    /**
     * @param  array<string, mixed>|null  $payload
     */
    private function shouldDispatch(string $status, ?array $payload): bool
    {
        if (! is_array($payload)) {
            return false;
        }

        return in_array($status, ['received', 'failed'], true);
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
