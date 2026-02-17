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
