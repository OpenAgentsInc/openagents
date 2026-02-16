<?php

namespace App\AI\Tools;

use App\Lightning\L402\L402Client;
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
        return 'Fetch an L402-protected HTTP resource, enforcing a strict maxSpendSats and server-side allowlist. Returns a bounded response preview + sha256 and a proof reference when paid.';
    }

    public function handle(Request $request): string
    {
        $url = (string) $request->string('url');
        $method = (string) $request->string('method', 'GET');
        $scope = (string) $request->string('scope', 'default');

        $maxSpendSats = $request->integer('maxSpendSats');

        $headersRaw = $request->array('headers');
        $headers = [];
        foreach ($headersRaw as $k => $v) {
            if (is_string($k) && is_string($v)) {
                $headers[$k] = $v;
            }
        }

        $body = $request->all()['body'] ?? null;
        if (! is_string($body)) {
            $body = null;
        }

        $result = resolve(L402Client::class)->fetch(
            url: $url,
            method: $method,
            headers: $headers,
            body: $body,
            maxSpendSats: $maxSpendSats,
            scope: $scope,
        );

        $json = json_encode($result, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        if (! is_string($json)) {
            throw new RuntimeException('Failed to JSON encode lightning_l402_fetch result.');
        }

        return $json;
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'url' => $schema
                ->string()
                ->description('The L402-protected URL to fetch.')
                ->required(),
            'method' => $schema
                ->string()
                ->description('HTTP method. Defaults to GET.')
                ->enum(['GET', 'POST', 'PUT', 'PATCH', 'DELETE'])
                ->default('GET'),
            'headers' => $schema
                ->object()
                ->description('Optional HTTP headers (string values).'),
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
        ];
    }
}
