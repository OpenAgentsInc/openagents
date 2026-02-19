<?php

namespace App\Support\Comms;

class ResendWebhookNormalizer
{
    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>|null
     */
    public function normalize(array $payload, ?string $externalEventId = null): ?array
    {
        $type = is_string($payload['type'] ?? null) ? (string) $payload['type'] : '';

        $deliveryState = match ($type) {
            'email.delivered' => 'delivered',
            'email.bounced' => 'bounced',
            'email.complained' => 'complained',
            'email.suppressed', 'email.unsubscribed' => 'unsubscribed',
            default => null,
        };

        if (! is_string($deliveryState)) {
            return null;
        }

        $data = is_array($payload['data'] ?? null) ? $payload['data'] : [];
        $tags = $this->normalizeTags($data['tags'] ?? []);

        $recipient = null;
        if (is_array($data['to'] ?? null) && isset($data['to'][0]) && is_string($data['to'][0])) {
            $recipient = (string) $data['to'][0];
        } elseif (is_string($data['email'] ?? null)) {
            $recipient = (string) $data['email'];
        }

        $occurredAt =
            (is_string($payload['created_at'] ?? null) ? (string) $payload['created_at'] : null)
            ?? (is_string($data['created_at'] ?? null) ? (string) $data['created_at'] : null)
            ?? now()->toISOString();

        $reason =
            (is_string($data['reason'] ?? null) ? (string) $data['reason'] : null)
            ?? (is_string($data['bounce']['reason'] ?? null) ? (string) $data['bounce']['reason'] : null)
            ?? (is_string($data['suppression']['reason'] ?? null) ? (string) $data['suppression']['reason'] : null);

        $messageId =
            (is_string($data['email_id'] ?? null) ? (string) $data['email_id'] : null)
            ?? (is_string($data['id'] ?? null) ? (string) $data['id'] : null);

        $eventId = $externalEventId;
        if (! is_string($eventId) || trim($eventId) === '') {
            $serialized = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
            $eventId = 'resend_'.hash('sha256', $serialized === false ? '{}' : $serialized);
        }

        $userId = null;
        $tagUserId = $tags['user_id'] ?? null;
        if (is_string($tagUserId) && ctype_digit($tagUserId) && (int) $tagUserId > 0) {
            $userId = (int) $tagUserId;
        }

        return [
            'event_id' => $eventId,
            'provider' => 'resend',
            'event_type' => $type,
            'delivery_state' => $deliveryState,
            'message_id' => $messageId,
            'integration_id' => is_string($tags['integration_id'] ?? null) ? (string) $tags['integration_id'] : null,
            'user_id' => $userId,
            'recipient' => $recipient,
            'occurred_at' => $occurredAt,
            'reason' => $reason,
            'payload' => [
                'raw_type' => $type,
                'tags' => $tags,
                'raw' => $payload,
            ],
        ];
    }

    /**
     * @return array<string, string>
     */
    private function normalizeTags(mixed $tagsValue): array
    {
        if (! is_array($tagsValue)) {
            return [];
        }

        $tags = [];

        foreach ($tagsValue as $entry) {
            if (is_array($entry)) {
                $name = $entry['name'] ?? null;
                $value = $entry['value'] ?? null;

                if (is_string($name) && $name !== '' && is_scalar($value)) {
                    $tags[$name] = (string) $value;
                }

                continue;
            }

            if (is_string($entry)) {
                $parts = explode(':', $entry, 2);
                if (count($parts) === 2 && trim($parts[0]) !== '') {
                    $tags[trim($parts[0])] = trim($parts[1]);
                }
            }
        }

        return $tags;
    }
}
