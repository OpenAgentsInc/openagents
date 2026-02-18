<?php

use App\AI\Tools\OpenAgentsApiTool;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Laravel\Ai\Tools\Request as ToolRequest;

test('openagents_api discover returns API endpoint metadata from openapi spec', function () {
    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'action' => 'discover',
        'path' => '/api/chats',
        'limit' => 5,
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['toolName'])->toBe('openagents_api');
    expect($result['status'])->toBe('ok');
    expect($result['action'])->toBe('discover');
    expect($result['returned'])->toBeGreaterThan(0);

    $first = $result['endpoints'][0] ?? null;
    expect($first)->toBeArray();
    expect($first['path'] ?? null)->toStartWith('/api/');
});

test('openagents_api request requires an authenticated user context', function () {
    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => '/api/me',
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('failed');
    expect($result['denyCode'])->toBe('auth_required');
});

test('openagents_api infers request action when method/path are provided', function () {
    config()->set('app.url', 'https://openagents.com.test');

    $user = User::factory()->create();
    $this->actingAs($user);

    Http::fake([
        'https://openagents.com.test/api/agent-payments/wallet' => Http::response([
            'data' => ['wallet' => ['walletId' => 'oa-user-1']],
        ], 200),
    ]);

    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'method' => 'POST',
        'path' => '/api/agent-payments/wallet',
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('ok');
    expect($result['action'])->toBe('request');
    expect($result['statusCode'])->toBe(200);
});

test('openagents_api request uses scoped sanctum token and deletes it after call', function () {
    config()->set('app.url', 'https://openagents.com.test');

    $user = User::factory()->create();
    $this->actingAs($user);

    Http::fake([
        'https://openagents.com.test/api/me' => Http::response([
            'data' => ['id' => $user->id, 'email' => $user->email],
        ], 200),
    ]);

    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => '/api/me',
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('ok');
    expect($result['statusCode'])->toBe(200);
    expect(data_get($result, 'response.json.data.id'))->toBe($user->id);

    Http::assertSent(function (\Illuminate\Http\Client\Request $request): bool {
        $auth = $request->header('Authorization')[0] ?? '';

        return $request->method() === 'GET'
            && $request->url() === 'https://openagents.com.test/api/me'
            && str_starts_with($auth, 'Bearer ')
            && strlen(substr($auth, 7)) > 10;
    });

    expect(DB::table('personal_access_tokens')->count())->toBe(0);
});

test('openagents_api request forwards incoming cookie header for maintenance/session continuity', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $incomingRequest = \Illuminate\Http\Request::create('https://openagents.com/chat', 'POST');
    $incomingRequest->headers->set('Cookie', 'laravel_maintenance=demo-bypass; openagents_session=demo-session');
    app()->instance('request', $incomingRequest);

    Http::fake([
        'https://openagents.com/api/me' => Http::response([
            'data' => ['id' => $user->id],
        ], 200),
    ]);

    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => '/api/me',
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('ok');

    Http::assertSent(function (\Illuminate\Http\Client\Request $request): bool {
        $cookie = $request->header('Cookie')[0] ?? '';

        return $request->method() === 'GET'
            && $request->url() === 'https://openagents.com/api/me'
            && str_contains($cookie, 'laravel_maintenance=demo-bypass')
            && str_contains($cookie, 'openagents_session=demo-session');
    });
});
test('openagents_api request blocks absolute and non-api paths', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $tool = new OpenAgentsApiTool;

    $absolute = json_decode($tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => 'https://example.com/api/me',
    ])), true);

    $nonApi = json_decode($tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => '/openapi.json',
    ])), true);

    expect($absolute['status'])->toBe('failed');
    expect($absolute['denyCode'])->toBe('path_not_allowed');

    expect($nonApi['status'])->toBe('failed');
    expect($nonApi['denyCode'])->toBe('path_not_allowed');
});

test('openagents_api request sanitizes http error payloads', function () {
    config()->set('app.url', 'https://openagents.com.test');

    $user = User::factory()->create();
    $this->actingAs($user);

    Http::fake([
        'https://openagents.com.test/api/agent-payments/balance' => Http::response([
            'message' => 'wallet_not_found',
            'trace' => ['should-not-be-returned'],
            'exception' => 'ExampleException',
        ], 404),
    ]);

    $tool = new OpenAgentsApiTool;

    $json = $tool->handle(new ToolRequest([
        'action' => 'request',
        'method' => 'GET',
        'path' => '/api/agent-payments/balance',
    ]));

    $result = json_decode($json, true);

    expect($result['status'])->toBe('http_error');
    expect($result['statusCode'])->toBe(404);
    expect(data_get($result, 'error.message'))->toBe('wallet_not_found');
    expect(data_get($result, 'response.json.message'))->toBe('wallet_not_found');
    expect(data_get($result, 'response.json.trace'))->toBeNull();
    expect(data_get($result, 'response.json.exception'))->toBeNull();
});
