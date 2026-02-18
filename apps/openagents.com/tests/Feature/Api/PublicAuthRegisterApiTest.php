<?php

use App\Models\Autopilot;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('auth.api_signup.enabled', false);
    config()->set('auth.api_signup.allowed_domains', []);
    config()->set('lightning.spark_executor.base_url', '');
    config()->set('lightning.spark_executor.auth_token', '');
    config()->set('lightning.agent_wallets.auto_provision_on_auth', true);
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

test('api signup auto provisions a wallet when spark executor is configured', function () {
    config()->set('auth.api_signup.enabled', true);
    config()->set('lightning.spark_executor.base_url', 'https://spark-executor.test');
    config()->set('lightning.spark_executor.auth_token', 'spark-token');

    Http::fake([
        'https://spark-executor.test/wallets/create' => Http::response([
            'ok' => true,
            'result' => [
                'mnemonic' => 'abandon ability able about above absent absorb abstract absurd abuse access accident',
                'sparkAddress' => 'staging-user-2@spark.wallet',
                'lightningAddress' => 'staging-user-2@lightning.openagents.com',
                'identityPubkey' => '02abc123',
                'balanceSats' => 0,
            ],
        ], 200),
    ]);

    $response = $this->postJson('/api/auth/register', [
        'email' => 'staging-user-2@staging.openagents.com',
        'name' => 'Staging User 2',
    ]);

    $response->assertCreated();

    $user = User::query()->where('email', 'staging-user-2@staging.openagents.com')->firstOrFail();

    expect(DB::table('user_spark_wallets')->where('user_id', $user->id)->exists())->toBeTrue();
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

test('api signup autopilot creation avoids handles held by soft deleted autopilots', function () {
    config()->set('auth.api_signup.enabled', true);

    $first = $this->postJson('/api/auth/register', [
        'email' => 'softdelete-owner@staging.openagents.com',
        'createAutopilot' => true,
    ]);

    $first->assertCreated();

    $firstUser = User::query()->where('email', 'softdelete-owner@staging.openagents.com')->firstOrFail();
    $firstAutopilot = Autopilot::query()->where('owner_user_id', $firstUser->id)->firstOrFail();
    $firstAutopilot->delete();

    $second = $this->postJson('/api/auth/register', [
        'email' => 'softdelete-next@staging.openagents.com',
        'createAutopilot' => true,
    ]);

    $second->assertCreated();

    $secondHandle = (string) $second->json('data.autopilot.handle');
    expect($secondHandle)->not->toBe('autopilot');
    expect(str_starts_with($secondHandle, 'autopilot-'))->toBeTrue();
});
