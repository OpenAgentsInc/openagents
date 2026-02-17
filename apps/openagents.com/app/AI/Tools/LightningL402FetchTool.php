<?php

namespace App\AI\Tools;

use App\Lightning\L402\L402Client;
use App\Lightning\L402\L402PolicyEnforcer;
use App\Lightning\L402\PendingL402ApprovalStore;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;
use RuntimeException;

class LightningL402FetchTool implements Tool
{
    public function name(): string
    {
        return 'lightning_l402_fetch';
    }

    public function description(): string
    {
        return 'Fetch an L402-protected HTTP resource with maxSpendMsats (maxSpendSats alias). By default this queues an approval intent and returns taskId; complete payment with lightning_l402_approve.';
    }

    public function handle(Request $request): string
    {
        $raw = $request->all();

        $presetName = trim((string) $request->string('endpointPreset', ''));
        $preset = $this->resolvePreset($presetName);

        $providedUrl = trim((string) $request->string('url', ''));
        $url = $providedUrl !== '' ? $providedUrl : ($preset['url'] ?? '');

        if (! is_string($url) || trim($url) === '') {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => 'url_missing',
                'requireApproval' => false,
                'approvalRequired' => false,
            ]);
        }

        $providedMethod = trim((string) $request->string('method', ''));
        $method = strtoupper($providedMethod !== '' ? $providedMethod : (string) ($preset['method'] ?? 'GET'));

        $scopeProvided = trim((string) $request->string('scope', ''));
        $scope = $scopeProvided !== '' ? $scopeProvided : (is_string($preset['scope'] ?? null) ? $preset['scope'] : 'default');

        $maxSpendMsats = $this->resolveMaxSpendMsats($raw);
        if (! is_int($maxSpendMsats) || $maxSpendMsats <= 0) {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'denyCode' => 'max_spend_missing',
                'requireApproval' => false,
                'approvalRequired' => false,
            ]);
        }

        $headers = [];
        $presetHeaders = isset($preset['headers']) && is_array($preset['headers']) ? $preset['headers'] : [];
        foreach ($presetHeaders as $k => $v) {
            if (is_string($k) && is_string($v)) {
                $headers[$k] = $v;
            }
        }

        $headersRaw = $request->array('headers');
        foreach ($headersRaw as $k => $v) {
            if (is_string($k) && is_string($v)) {
                $headers[$k] = $v;
            }
        }

        $body = $request->all()['body'] ?? null;
        if (! is_string($body)) {
            $body = null;
        }

        if ($body === null && isset($preset['body']) && is_string($preset['body'])) {
            $body = $preset['body'];
        }

        $userId = $this->resolveUserId();
        $requestedRequireApproval = $this->resolveRequireApproval($raw);

        $policyDecision = resolve(L402PolicyEnforcer::class)->evaluate(
            url: $url,
            requestedMaxSpendMsats: $maxSpendMsats,
            requestedRequireApproval: $requestedRequireApproval,
        );

        $effectiveRequireApproval = (bool) ($policyDecision['effectiveRequireApproval'] ?? $requestedRequireApproval);
        $effectiveMaxSpendMsats = (int) ($policyDecision['effectiveMaxSpendMsats'] ?? $maxSpendMsats);
        $effectiveMaxSpendSats = $this->safeSatsFromMsats($effectiveMaxSpendMsats);

        $denyCode = $policyDecision['denyCode'] ?? null;
        if (is_string($denyCode) && $denyCode !== '') {
            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'blocked',
                'paid' => false,
                'cacheHit' => false,
                'cacheStatus' => 'none',
                'paymentBackend' => (string) config('lightning.l402.invoice_payer', 'unknown'),
                'host' => $this->hostFromUrl($url),
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendMsats' => $effectiveMaxSpendMsats,
                'maxSpendSats' => $effectiveMaxSpendSats,
                'requireApproval' => $effectiveRequireApproval,
                'approvalRequired' => $effectiveRequireApproval,
                'policySource' => $policyDecision['policySource'] ?? 'config',
                'denyCode' => $denyCode,
                'denyReason' => $policyDecision['denyReason'] ?? null,
            ]);
        }

        if ($effectiveRequireApproval) {
            $taskPayload = [
                'url' => $url,
                'method' => $method,
                'headers' => $headers,
                'body' => $body,
                'maxSpendMsats' => $effectiveMaxSpendMsats,
                'maxSpendSats' => $effectiveMaxSpendSats,
                'scope' => $scope,
                'preset' => $presetName !== '' ? $presetName : null,
                'createdAt' => now()->toISOString(),
                'userId' => $userId,
                'autopilotId' => is_string($policyDecision['autopilotId'] ?? null) ? (string) $policyDecision['autopilotId'] : null,
            ];

            $taskId = resolve(PendingL402ApprovalStore::class)->create($taskPayload);

            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'approval_requested',
                'requireApproval' => true,
                'approvalRequired' => true,
                'taskId' => $taskId,
                'paid' => false,
                'cacheHit' => false,
                'cacheStatus' => 'none',
                'paymentBackend' => (string) config('lightning.l402.invoice_payer', 'unknown'),
                'host' => $this->hostFromUrl($url),
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $effectiveMaxSpendSats,
                'maxSpendMsats' => $effectiveMaxSpendMsats,
                'policySource' => $policyDecision['policySource'] ?? 'config',
                'responseStatusCode' => null,
            ]);
        }

        $context = [
            'userId' => $userId,
            'autopilotId' => is_string($policyDecision['autopilotId'] ?? null) ? (string) $policyDecision['autopilotId'] : null,
        ];

        if (($policyDecision['policySource'] ?? 'config') === 'autopilot' && is_array($policyDecision['allowedHosts'] ?? null)) {
            $context['allowedHosts'] = $policyDecision['allowedHosts'];
        }

        $result = resolve(L402Client::class)->fetch(
            url: $url,
            method: $method,
            headers: $headers,
            body: $body,
            maxSpendSats: $effectiveMaxSpendSats,
            scope: $scope,
            context: $context,
        );

        $result['toolName'] = 'lightning_l402_fetch';
        $result['requireApproval'] = false;
        $result['approvalRequired'] = false;
        $result['maxSpendMsats'] = $effectiveMaxSpendMsats;
        $result['maxSpendSats'] = $effectiveMaxSpendSats;
        $result['policySource'] = $policyDecision['policySource'] ?? 'config';

        return $this->encode($result);
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'endpointPreset' => $schema
                ->string()
                ->description('Optional named endpoint preset from server config (e.g. sats4ai, fake). When provided, omitted url/method/headers/body/scope are filled from preset.'),
            'url' => $schema
                ->string()
                ->description('The L402-protected URL to fetch. Optional if endpointPreset provides url.'),
            'method' => $schema
                ->string()
                ->description('HTTP method. Defaults to preset method or GET.')
                ->enum(['GET', 'POST', 'PUT', 'PATCH', 'DELETE'])
                ->default('GET'),
            'headers' => $schema
                ->object()
                ->description('Optional HTTP headers (string values). Merged over preset headers.'),
            'body' => $schema
                ->string()
                ->description('Optional request body as a raw string (use JSON when posting to JSON APIs).'),
            'maxSpendMsats' => $schema
                ->integer()
                ->description('Canonical hard cap in millisats for this single request. Alias: maxSpendSats.'),
            'maxSpendSats' => $schema
                ->integer()
                ->description('Deprecated alias for maxSpendMsats. Used only when maxSpendMsats is omitted.'),
            'scope' => $schema
                ->string()
                ->description('Credential cache namespace (e.g. ep212.sats4ai).')
                ->default('default'),
            'requireApproval' => $schema
                ->boolean()
                ->description('Canonical approval toggle. When true (default), queue payment intent and require lightning_l402_approve(taskId) before spending. Alias: approvalRequired.')
                ->default(true),
            'approvalRequired' => $schema
                ->boolean()
                ->description('Deprecated alias for requireApproval. Used only when requireApproval is omitted.'),
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function resolvePreset(string $presetName): array
    {
        if ($presetName === '') {
            return [];
        }

        $preset = config('lightning.demo_presets.'.$presetName);

        return is_array($preset) ? $preset : [];
    }

    /**
     * @param  array<string, mixed>  $raw
     */
    private function resolveMaxSpendMsats(array $raw): ?int
    {
        if (array_key_exists('maxSpendMsats', $raw) && is_numeric($raw['maxSpendMsats'])) {
            return max(0, (int) $raw['maxSpendMsats']);
        }

        if (array_key_exists('maxSpendSats', $raw) && is_numeric($raw['maxSpendSats'])) {
            return $this->safeMsatsFromSats((int) $raw['maxSpendSats']);
        }

        return null;
    }

    /**
     * @param  array<string, mixed>  $raw
     */
    private function resolveRequireApproval(array $raw): bool
    {
        if (array_key_exists('requireApproval', $raw)) {
            return filter_var($raw['requireApproval'], FILTER_VALIDATE_BOOL, FILTER_NULL_ON_FAILURE) ?? true;
        }

        if (array_key_exists('approvalRequired', $raw)) {
            return filter_var($raw['approvalRequired'], FILTER_VALIDATE_BOOL, FILTER_NULL_ON_FAILURE) ?? true;
        }

        return true;
    }

    private function hostFromUrl(string $url): ?string
    {
        $host = parse_url($url, PHP_URL_HOST);

        return is_string($host) ? strtolower($host) : null;
    }

    private function safeMsatsFromSats(int $sats): int
    {
        $sats = max(0, $sats);

        if ($sats > intdiv(PHP_INT_MAX, 1000)) {
            return PHP_INT_MAX;
        }

        return $sats * 1000;
    }

    private function safeSatsFromMsats(int $msats): int
    {
        $msats = max(0, $msats);

        if ($msats === 0) {
            return 0;
        }

        return intdiv($msats + 999, 1000);
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
            throw new RuntimeException('Failed to JSON encode lightning_l402_fetch result.');
        }

        return $json;
    }
}
