<?php

use App\Models\User;
use App\Models\UserIntegration;
use Illuminate\Support\Facades\DB;

test('integrations settings page is displayed', function () {
    $user = User::factory()->create();

    $response = $this
        ->actingAs($user)
        ->get(route('settings.integrations.edit'));

    $response->assertOk();
});

test('user can connect resend integration and key is stored encrypted at rest', function () {
    $user = User::factory()->create();

    $rawKey = 're_test_1234567890';

    $response = $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.upsert'), [
            'resend_api_key' => $rawKey,
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('settings.integrations.edit'));

    $integration = UserIntegration::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->first();

    expect($integration)->not->toBeNull();
    expect((string) $integration->status)->toBe('active');
    expect((string) $integration->secret_last4)->toBe('7890');

    // Cast returns decrypted value.
    expect((string) $integration->encrypted_secret)->toBe($rawKey);

    $rawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $rawStored)->not->toBe($rawKey);

    $page = $this->actingAs($user)->get(route('settings.integrations.edit'));
    $page->assertOk();
    $page->assertDontSee($rawKey);
    $page->assertSee('7890');
});

test('disconnecting resend clears secret and marks integration inactive', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_test_1234567890',
        'secret_last4' => '7890',
        'secret_fingerprint' => hash('sha256', 're_test_1234567890'),
        'connected_at' => now(),
    ]);

    $response = $this
        ->actingAs($user)
        ->delete(route('settings.integrations.resend.disconnect'));

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('settings.integrations.edit'));

    $integration = UserIntegration::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->first();

    expect((string) $integration->status)->toBe('inactive');
    expect($integration->encrypted_secret)->toBeNull();
    expect($integration->secret_last4)->toBeNull();
});

test('disconnect action only affects authenticated users integration', function () {
    $userA = User::factory()->create();
    $userB = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $userA->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_user_a_1234',
        'secret_last4' => '1234',
        'secret_fingerprint' => hash('sha256', 're_user_a_1234'),
        'connected_at' => now(),
    ]);

    UserIntegration::query()->create([
        'user_id' => $userB->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_user_b_5678',
        'secret_last4' => '5678',
        'secret_fingerprint' => hash('sha256', 're_user_b_5678'),
        'connected_at' => now(),
    ]);

    $this->actingAs($userA)->delete(route('settings.integrations.resend.disconnect'));

    $integrationA = UserIntegration::query()->where('user_id', $userA->id)->where('provider', 'resend')->first();
    $integrationB = UserIntegration::query()->where('user_id', $userB->id)->where('provider', 'resend')->first();

    expect((string) $integrationA->status)->toBe('inactive');
    expect((string) $integrationB->status)->toBe('active');
    expect((string) $integrationB->secret_last4)->toBe('5678');
});

test('test resend endpoint requires active integration', function () {
    $user = User::factory()->create();

    $response = $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.test'));

    $response->assertSessionHasErrors(['resend']);

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_test_1234567890',
        'secret_last4' => '7890',
        'secret_fingerprint' => hash('sha256', 're_test_1234567890'),
        'connected_at' => now(),
    ]);

    $okResponse = $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.test'));

    $okResponse
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('settings.integrations.edit'));
});
