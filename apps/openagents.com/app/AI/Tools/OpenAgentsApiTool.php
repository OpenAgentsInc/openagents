<?php

namespace App\AI\Tools;

use App\Models\User;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;
use Throwable;
use Vyuldashev\LaravelOpenApi\Generator;

class OpenAgentsApiTool implements Tool
{
    private const TOOL_NAME = 'openagents_api';

    /** @var array<int, string> */
    private const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];

    public function name(): string
    {
        return self::TOOL_NAME;
    }

    public function description(): string
    {
        return 'Use OpenAgents REST API. Run action=discover to inspect endpoints from /openapi.json, then action=request to call relative /api/* endpoints using the current user auth context.';
    }

    public function handle(Request $request): string
    {
        $action = strtolower(trim((string) $request->string('action', 'discover')));

        return $this->encode(match ($action) {
            'discover' => $this->discover($request),
            'request' => $this->request($request),
            default => [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => $action,
                'denyCode' => 'invalid_action',
                'message' => 'action must be discover or request.',
            ],
        });
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'action' => $schema
                ->string()
                ->description('discover: inspect endpoints in /openapi.json. request: execute one /api/* endpoint with authenticated user context.')
                ->enum(['discover', 'request'])
                ->default('discover')
                ->required(),
            'method' => $schema
                ->string()
                ->description('Required for action=request. HTTP method.')
                ->enum(self::HTTP_METHODS),
            'path' => $schema
                ->string()
                ->description('For action=request: relative API path like /api/me (absolute URLs are blocked). For action=discover: optional substring filter.'),
            'query' => $schema
                ->object()
                ->description('Optional query parameters for action=request.'),
            'json' => $schema
                ->object()
                ->description('Optional JSON body for action=request (POST/PUT/PATCH).'),
            'body' => $schema
                ->string()
                ->description('Optional raw request body for action=request when json is not provided.'),
            'headers' => $schema
                ->object()
                ->description('Optional extra request headers for action=request.'),
            'tag' => $schema
                ->string()
                ->description('Optional OpenAPI tag filter for action=discover.'),
            'limit' => $schema
                ->integer()
                ->description('Max endpoints returned for discover (default 100, max 100).')
                ->default(100),
            'includeSchema' => $schema
                ->boolean()
                ->description('When true, discover includes requestBody and parameters snippets.')
                ->default(false),
            'timeoutMs' => $schema
                ->integer()
                ->description('Network timeout for action=request in milliseconds (default 15000, max 60000).')
                ->default(15000),
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function discover(Request $request): array
    {
        $spec = $this->loadOpenApiSpec();

        if (! is_array($spec) || ! is_array($spec['paths'] ?? null)) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'discover',
                'denyCode' => 'openapi_unavailable',
                'message' => 'Could not load /openapi.json',
            ];
        }

        $methodFilter = strtoupper(trim((string) $request->string('method', '')));
        if ($methodFilter !== '' && ! in_array($methodFilter, self::HTTP_METHODS, true)) {
            $methodFilter = '';
        }

        $pathFilter = strtolower(trim((string) $request->string('path', '')));
        $tagFilter = strtolower(trim((string) $request->string('tag', '')));
        $includeSchema = (bool) $request->boolean('includeSchema', false);

        $limit = (int) $request->integer('limit', 100);
        if ($limit <= 0) {
            $limit = 100;
        }
        $limit = min($limit, 100);

        $securityRoot = is_array($spec['security'] ?? null) ? $spec['security'] : [];

        /** @var array<int, array<string, mixed>> $endpoints */
        $endpoints = [];

        foreach ($spec['paths'] as $path => $operations) {
            if (! is_string($path) || ! is_array($operations)) {
                continue;
            }

            if (! str_starts_with($path, '/api/')) {
                continue;
            }

            if ($pathFilter !== '' && ! str_contains(strtolower($path), $pathFilter)) {
                continue;
            }

            foreach ($operations as $method => $operation) {
                if (! is_string($method) || ! is_array($operation)) {
                    continue;
                }

                $normalizedMethod = strtoupper($method);
                if (! in_array($normalizedMethod, self::HTTP_METHODS, true)) {
                    continue;
                }

                if ($methodFilter !== '' && $normalizedMethod !== $methodFilter) {
                    continue;
                }

                $tags = collect($operation['tags'] ?? [])
                    ->filter(fn (mixed $tag): bool => is_string($tag) && trim($tag) !== '')
                    ->map(fn (string $tag): string => trim($tag))
                    ->values()
                    ->all();

                if ($tagFilter !== '') {
                    $tagMatches = collect($tags)
                        ->contains(fn (string $tag): bool => strtolower($tag) === $tagFilter);

                    if (! $tagMatches) {
                        continue;
                    }
                }

                $endpoint = [
                    'method' => $normalizedMethod,
                    'path' => $path,
                    'summary' => $this->nullableString($operation['summary'] ?? null),
                    'description' => $this->nullableString($operation['description'] ?? null),
                    'tags' => $tags,
                    'requiresAuth' => $this->requiresAuth($operation, $securityRoot),
                ];

                if ($includeSchema) {
                    if (is_array($operation['parameters'] ?? null)) {
                        $endpoint['parameters'] = $operation['parameters'];
                    }

                    if (is_array($operation['requestBody'] ?? null)) {
                        $endpoint['requestBody'] = $operation['requestBody'];
                    }
                }

                $endpoints[] = $endpoint;
            }
        }

        usort($endpoints, function (array $a, array $b): int {
            $pathCmp = strcmp((string) ($a['path'] ?? ''), (string) ($b['path'] ?? ''));
            if ($pathCmp !== 0) {
                return $pathCmp;
            }

            return strcmp((string) ($a['method'] ?? ''), (string) ($b['method'] ?? ''));
        });

        $total = count($endpoints);
        $trimmed = array_slice($endpoints, 0, $limit);

        return [
            'toolName' => self::TOOL_NAME,
            'status' => 'ok',
            'action' => 'discover',
            'apiTitle' => $this->nullableString(data_get($spec, 'info.title')),
            'apiVersion' => $this->nullableString(data_get($spec, 'info.version')),
            'server' => $this->nullableString(data_get($spec, 'servers.0.url')),
            'totalEndpoints' => $total,
            'returned' => count($trimmed),
            'limit' => $limit,
            'endpoints' => $trimmed,
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function request(Request $request): array
    {
        $user = auth()->user();
        if (! $user instanceof User) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'request',
                'denyCode' => 'auth_required',
                'message' => 'Authenticated user context is required for openagents_api request action.',
            ];
        }

        $method = strtoupper(trim((string) $request->string('method', '')));
        if (! in_array($method, self::HTTP_METHODS, true)) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'request',
                'denyCode' => 'invalid_method',
                'message' => 'method must be one of: '.implode(', ', self::HTTP_METHODS),
            ];
        }

        $path = $this->normalizeApiPath((string) $request->string('path', ''));
        if ($path === null) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'request',
                'denyCode' => 'path_not_allowed',
                'message' => 'path must be a relative /api/* path (absolute URLs are blocked).',
            ];
        }

        $timeoutMs = (int) $request->integer('timeoutMs', 15000);
        $timeoutMs = max(1000, min(60000, $timeoutMs));

        $queryFromPath = $this->queryFromPath((string) $request->string('path', ''));
        $query = array_replace_recursive($queryFromPath, $this->normalizeMap($request->all()['query'] ?? null));

        $jsonPayload = $this->normalizeMap($request->all()['json'] ?? null);
        $rawBody = $this->nullableString($request->all()['body'] ?? null);
        $headers = $this->normalizeHeaderMap($request->all()['headers'] ?? null);

        $baseUrl = $this->resolveApiBaseUrl();
        if ($baseUrl === null) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'request',
                'denyCode' => 'base_url_unavailable',
                'message' => 'Unable to resolve API base URL.',
            ];
        }

        $token = $user->createToken('openagents-api-tool', ['*']);

        try {
            $options = [
                'headers' => array_merge([
                    'Accept' => 'application/json',
                ], $headers),
            ];

            $incomingCookieHeader = request()?->header('Cookie');
            if (is_string($incomingCookieHeader) && trim($incomingCookieHeader) !== '') {
                $options['headers']['Cookie'] = $incomingCookieHeader;
            }

            if ($query !== []) {
                $options['query'] = $query;
            }

            if (in_array($method, ['POST', 'PUT', 'PATCH'], true)) {
                if ($jsonPayload !== []) {
                    $options['json'] = $jsonPayload;
                } elseif (is_string($rawBody) && $rawBody !== '') {
                    $options['body'] = $rawBody;
                }
            }

            $response = Http::baseUrl($baseUrl)
                ->withToken($token->plainTextToken)
                ->timeout(max(1, (int) ceil($timeoutMs / 1000)))
                ->send($method, $path, $options);
        } catch (Throwable $e) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'request',
                'method' => $method,
                'path' => $path,
                'denyCode' => 'request_failed',
                'message' => $e->getMessage(),
            ];
        } finally {
            $token->accessToken?->delete();
        }

        $responseBody = (string) $response->body();
        $decoded = json_decode($responseBody, true);
        $decodedOk = json_last_error() === JSON_ERROR_NONE && is_array($decoded);

        return [
            'toolName' => self::TOOL_NAME,
            'status' => $response->successful() ? 'ok' : 'http_error',
            'action' => 'request',
            'method' => $method,
            'path' => $path,
            'statusCode' => $response->status(),
            'ok' => $response->successful(),
            'response' => [
                'contentType' => $this->nullableString($response->header('Content-Type')),
                'bytes' => strlen($responseBody),
                'json' => $decodedOk ? $decoded : null,
                'bodyPreview' => $decodedOk ? null : Str::limit($responseBody, 2000, '...'),
            ],
        ];
    }

    /**
     * @return array<string, mixed>|null
     */
    private function loadOpenApiSpec(): ?array
    {
        $path = public_path('openapi.json');
        if (is_file($path) && is_readable($path)) {
            $contents = file_get_contents($path);
            if (is_string($contents) && $contents !== '') {
                $decoded = json_decode($contents, true);
                if (json_last_error() === JSON_ERROR_NONE && is_array($decoded)) {
                    return $decoded;
                }
            }
        }

        $generated = resolve(Generator::class)
            ->generate()
            ->toJson(JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);

        $decoded = json_decode($generated, true);

        return json_last_error() === JSON_ERROR_NONE && is_array($decoded)
            ? $decoded
            : null;
    }

    private function requiresAuth(array $operation, array $securityRoot): bool
    {
        if (array_key_exists('security', $operation)) {
            $security = $operation['security'];
            if (is_array($security)) {
                return $security !== [];
            }
        }

        return $securityRoot !== [];
    }

    private function normalizeApiPath(string $rawPath): ?string
    {
        $candidate = trim($rawPath);
        if ($candidate === '') {
            return null;
        }

        if (preg_match('/^https?:\/\//i', $candidate) === 1) {
            return null;
        }

        if (! str_starts_with($candidate, '/')) {
            $candidate = '/'.$candidate;
        }

        $path = parse_url($candidate, PHP_URL_PATH);
        if (! is_string($path) || $path === '') {
            return null;
        }

        if (! str_starts_with($path, '/api/')) {
            return null;
        }

        return $path;
    }

    /**
     * @return array<string, mixed>
     */
    private function queryFromPath(string $rawPath): array
    {
        $query = parse_url($rawPath, PHP_URL_QUERY);
        if (! is_string($query) || $query === '') {
            return [];
        }

        $parsed = [];
        parse_str($query, $parsed);

        return $this->normalizeMap($parsed);
    }

    /**
     * @return array<string, mixed>
     */
    private function normalizeMap(mixed $value): array
    {
        if (! is_array($value)) {
            return [];
        }

        $out = [];

        foreach ($value as $key => $item) {
            if (! is_string($key) || trim($key) === '') {
                continue;
            }

            if (is_scalar($item) || $item === null) {
                $out[$key] = $item;

                continue;
            }

            if (is_array($item)) {
                $out[$key] = $this->normalizeListOrMap($item);
            }
        }

        return $out;
    }

    /**
     * @return array<int|string, mixed>
     */
    private function normalizeListOrMap(array $value): array
    {
        $out = [];

        foreach ($value as $key => $item) {
            if (is_scalar($item) || $item === null) {
                $out[$key] = $item;

                continue;
            }

            if (is_array($item)) {
                $out[$key] = $this->normalizeListOrMap($item);
            }
        }

        return $out;
    }

    /**
     * @return array<string, string>
     */
    private function normalizeHeaderMap(mixed $value): array
    {
        if (! is_array($value)) {
            return [];
        }

        $out = [];

        foreach ($value as $key => $item) {
            if (! is_string($key) || trim($key) === '') {
                continue;
            }

            if (is_string($item) || is_numeric($item)) {
                $out[$key] = (string) $item;
            }
        }

        return $out;
    }

    private function resolveApiBaseUrl(): ?string
    {
        $currentRequest = request();
        if ($currentRequest !== null) {
            $origin = trim((string) $currentRequest->getSchemeAndHttpHost());
            if ($origin !== '') {
                return rtrim($origin, '/');
            }
        }

        $configured = trim((string) config('app.url'));

        return $configured !== '' ? rtrim($configured, '/') : null;
    }

    private function nullableString(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed === '' ? null : $trimmed;
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function encode(array $payload): string
    {
        return json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) ?: '{"status":"failed","denyCode":"encoding_failed"}';
    }
}
