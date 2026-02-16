<?php

namespace App\AI\Tools;

use App\Lightning\L402\L402Client;
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
        return 'Fetch an L402-protected HTTP resource with strict maxSpendSats + allowlist. By default this queues an approval intent and returns taskId; complete payment with lightning_l402_approve.';
    }

    public function handle(Request $request): string
    {
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
                'approvalRequired' => false,
            ]);
        }

        $providedMethod = trim((string) $request->string('method', ''));
        $method = strtoupper($providedMethod !== '' ? $providedMethod : (string) ($preset['method'] ?? 'GET'));

        $scopeProvided = trim((string) $request->string('scope', ''));
        $scope = $scopeProvided !== '' ? $scopeProvided : (is_string($preset['scope'] ?? null) ? $preset['scope'] : 'default');

        $maxSpendSats = $request->integer('maxSpendSats');

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

        $approvalRequired = $request->boolean('approvalRequired', true);

        if ($approvalRequired) {
            $taskPayload = [
                'url' => $url,
                'method' => $method,
                'headers' => $headers,
                'body' => $body,
                'maxSpendSats' => $maxSpendSats,
                'scope' => $scope,
                'preset' => $presetName !== '' ? $presetName : null,
                'createdAt' => now()->toISOString(),
            ];

            $taskId = resolve(PendingL402ApprovalStore::class)->create($taskPayload);

            return $this->encode([
                'toolName' => 'lightning_l402_fetch',
                'status' => 'approval_requested',
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
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $this->safeMsatsFromSats($maxSpendSats),
                'responseStatusCode' => null,
            ]);
        }

        $result = resolve(L402Client::class)->fetch(
            url: $url,
            method: $method,
            headers: $headers,
            body: $body,
            maxSpendSats: $maxSpendSats,
            scope: $scope,
        );

        $result['toolName'] = 'lightning_l402_fetch';
        $result['approvalRequired'] = false;

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
            'maxSpendSats' => $schema
                ->integer()
                ->description('Hard cap in sats for this single request.')
                ->required(),
            'scope' => $schema
                ->string()
                ->description('Credential cache namespace (e.g. ep212.sats4ai).')
                ->default('default'),
            'approvalRequired' => $schema
                ->boolean()
                ->description('When true (default), queue payment intent and require lightning_l402_approve(taskId) before spending.')
                ->default(true),
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
