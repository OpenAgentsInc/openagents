<?php

use App\Models\User;
use Illuminate\Testing\TestResponse;

/**
 * @return array<string, mixed>
 */
function dashboardInertiaPayload(TestResponse $response): array
{
    $html = (string) $response->getContent();

    preg_match('/data-page="([^"]+)"/', $html, $matches);

    expect($matches)->toHaveKey(1);

    $json = html_entity_decode((string) $matches[1], ENT_QUOTES);

    /** @var array<string, mixed> $payload */
    $payload = json_decode($json, true, 512, JSON_THROW_ON_ERROR);

    return $payload;
}

test('legacy chat route is decommissioned for guests', function () {
    $this->get('/chat')->assertNotFound();
});

test('legacy chat route is decommissioned for authenticated users', function () {
    $this->actingAs(User::factory()->create());

    $this->get('/chat')->assertNotFound();
});

test('home rehydrates authenticated user from chat auth session key', function () {
    $user = User::factory()->create([
        'email' => 'rehydrate-user@openagents.com',
    ]);

    $response = $this->withSession([
        'chat.auth_user_id' => (int) $user->id,
    ])->get('/');

    $response->assertOk();

    $payload = dashboardInertiaPayload($response);

    expect($payload['props']['auth']['user']['email'] ?? null)->toBe($user->email);
    expect(auth()->check())->toBeTrue();
});
