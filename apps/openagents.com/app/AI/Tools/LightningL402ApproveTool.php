<?php

namespace App\AI\Tools;

use App\Lightning\L402\L402Client;
use App\Lightning\L402\PendingL402ApprovalStore;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;
use RuntimeException;

class LightningL402ApproveTool implements Tool
{
    public function name(): string
    {
        return 'lightning_l402_approve';
    }

    public function description(): string
    {
        return 'Approve and execute a previously queued L402 payment intent by taskId.';
    }

    public function handle(Request $request): string
    {
        $taskId = trim((string) $request->string('taskId'));

        if ($taskId === '') {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => 'task_id_missing',
                'approvalRequired' => false,
            ]);
        }

        $store = resolve(PendingL402ApprovalStore::class);
        $consumed = $store->consume($taskId);

        if (($consumed['status'] ?? 'missing') !== 'consumed' || ! isset($consumed['payload']) || ! is_array($consumed['payload'])) {
            $denyCode = ($consumed['status'] ?? 'missing') === 'expired' ? 'task_expired' : 'task_not_found';

            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => $denyCode,
                'approvalRequired' => false,
                'taskId' => $taskId,
            ]);
        }

        $payload = $consumed['payload'];

        $currentUserId = $this->resolveUserId();
        $expectedUserId = isset($payload['userId']) && is_numeric($payload['userId']) ? (int) $payload['userId'] : null;
        if (is_int($expectedUserId) && is_int($currentUserId) && $expectedUserId !== $currentUserId) {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => 'task_user_mismatch',
                'approvalRequired' => false,
                'taskId' => $taskId,
            ]);
        }

        $url = isset($payload['url']) && is_string($payload['url']) ? $payload['url'] : null;
        $method = isset($payload['method']) && is_string($payload['method']) ? $payload['method'] : 'GET';
        $headers = isset($payload['headers']) && is_array($payload['headers']) ? $payload['headers'] : [];
        $body = isset($payload['body']) && is_string($payload['body']) ? $payload['body'] : null;
        $scope = isset($payload['scope']) && is_string($payload['scope']) ? $payload['scope'] : 'default';
        $maxSpendSats = isset($payload['maxSpendSats']) && is_numeric($payload['maxSpendSats']) ? (int) $payload['maxSpendSats'] : null;

        if (! is_string($url) || $url === '' || ! is_int($maxSpendSats)) {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => 'task_payload_invalid',
                'approvalRequired' => false,
                'taskId' => $taskId,
            ]);
        }

        $result = resolve(L402Client::class)->fetch(
            url: $url,
            method: $method,
            headers: $headers,
            body: $body,
            maxSpendSats: $maxSpendSats,
            scope: $scope,
            context: [
                'userId' => $currentUserId ?? $expectedUserId,
            ],
        );

        $result['toolName'] = 'lightning_l402_fetch';
        $result['approvalRequired'] = false;
        $result['taskId'] = $taskId;

        return $this->encode($result);
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'taskId' => $schema
                ->string()
                ->description('Task id returned by lightning_l402_fetch when approval was requested.')
                ->required(),
        ];
    }

    private function resolveUserId(): ?int
    {
        $id = auth()->id();

        if (is_int($id)) {
            return $id;
        }

        if (is_numeric($id)) {
            return (int) $id;
        }

        return null;
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function encode(array $payload): string
    {
        $json = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        if (! is_string($json)) {
            throw new RuntimeException('Failed to JSON encode lightning_l402_approve result.');
        }

        return $json;
    }
}
