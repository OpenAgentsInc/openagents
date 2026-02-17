<?php

use App\Models\Autopilot;
use App\Models\User;

beforeEach(function () {
    config()->set('auth.api_signup.enabled', false);
    config()->set('auth.api_signup.allowed_domains', []);
});

test('api signup endpoint is disabled by default', function () {
    $response = $this->postJson('/api/auth/register', [
        'email' => 'staging-user-1@staging.openagents.com',
        'name' => 'Staging User 1',
    ]);

    $response->assertNotFound();
});

test('api signup creates user and returns sanctum bearer token when enabled', function () {
    config()->set('auth.api_signup.enabled', true);

    $response = $this->postJson('/api/auth/register', [
        'email' => 'staging-user-1@staging.openagents.com',
        'name' => 'Staging User 1',
        'tokenName' => 'staging-e2e',
    ]);

    $response
        ->assertCreated()
        ->assertJsonPath('data.created', true)
        ->assertJsonPath('data.user.email', 'staging-user-1@staging.openagents.com')
        ->assertJsonPath('data.tokenName', 'staging-e2e');

    $token = $response->json('data.token');

    expect(is_string($token) && str_contains($token, '|'))->toBeTrue();

    $me = $this->withToken($token)->getJson('/api/me');
    $me->assertOk()->assertJsonPath('data.user.email', 'staging-user-1@staging.openagents.com');

    expect(User::query()->where('email', 'staging-user-1@staging.openagents.com')->exists())->toBeTrue();
});

test('api signup enforces configured email domain allowlist', function () {
    config()->set('auth.api_signup.enabled', true);
    config()->set('auth.api_signup.allowed_domains', ['staging.openagents.com']);

    $blocked = $this->postJson('/api/auth/register', [
        'email' => 'blocked@example.com',
    ]);

    $blocked
        ->assertStatus(422)
        ->assertJsonValidationErrors(['email']);

    $allowed = $this->postJson('/api/auth/register', [
        'email' => 'allowed@staging.openagents.com',
    ]);

    $allowed->assertCreated();
});

test('api signup can create default autopilot for new account', function () {
    config()->set('auth.api_signup.enabled', true);

    $response = $this->postJson('/api/auth/register', [
        'email' => 'creator@staging.openagents.com',
        'createAutopilot' => true,
        'autopilotDisplayName' => 'Creator Agent',
    ]);

    $response
        ->assertCreated()
        ->assertJsonPath('data.autopilot.displayName', 'Creator Agent');

    $user = User::query()->where('email', 'creator@staging.openagents.com')->firstOrFail();

    expect(Autopilot::query()->where('owner_user_id', $user->id)->count())->toBe(1);
});
