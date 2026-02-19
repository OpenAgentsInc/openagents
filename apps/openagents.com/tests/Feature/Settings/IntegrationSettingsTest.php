<?php

use App\Models\User;
use App\Models\UserIntegration;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;

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

test('google oauth redirect route starts consent flow with session state', function () {
    $user = User::factory()->create();

    config()->set('services.google.client_id', 'google-client-id');
    config()->set('services.google.redirect_uri', 'https://openagents.test/settings/integrations/google/callback');
    config()->set('services.google.scopes', 'https://www.googleapis.com/auth/gmail.readonly');

    $response = $this
        ->actingAs($user)
        ->get(route('settings.integrations.google.redirect'));

    $response->assertRedirect();
    expect((string) $response->headers->get('Location'))->toContain('https://accounts.google.com/o/oauth2/v2/auth');
    $response->assertSessionHas('settings.integrations.google_oauth_state');
});

test('google oauth callback exchanges code and stores encrypted integration payload', function () {
    $user = User::factory()->create();

    config()->set('services.google.client_id', 'google-client-id');
    config()->set('services.google.client_secret', 'google-client-secret');
    config()->set('services.google.redirect_uri', 'https://openagents.test/settings/integrations/google/callback');

    Http::fake([
        'https://oauth2.googleapis.com/token' => Http::response([
            'access_token' => 'ya29.access-token',
            'refresh_token' => '1//refresh_token_1234',
            'scope' => 'https://www.googleapis.com/auth/gmail.readonly',
            'token_type' => 'Bearer',
            'expires_in' => 3600,
        ], 200),
    ]);

    $response = $this
        ->actingAs($user)
        ->withSession([
            'settings.integrations.google_oauth_state' => 'state-123',
        ])
        ->get(route('settings.integrations.google.callback', [
            'code' => 'auth-code-123',
            'state' => 'state-123',
        ]));

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('settings.integrations.edit'));

    $integration = UserIntegration::query()
        ->where('user_id', $user->id)
        ->where('provider', 'google')
        ->first();

    expect($integration)->not->toBeNull();
    expect((string) $integration->status)->toBe('active');
    expect((string) $integration->secret_last4)->toBe('1234');

    $decryptedPayload = json_decode((string) $integration->encrypted_secret, true);
    expect(is_array($decryptedPayload))->toBeTrue();
    expect((string) ($decryptedPayload['refresh_token'] ?? ''))->toBe('1//refresh_token_1234');
    expect((string) ($decryptedPayload['access_token'] ?? ''))->toBe('ya29.access-token');
    expect((string) ($decryptedPayload['integration_id'] ?? ''))->toBe('gmail.primary');

    $rawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $rawStored)->not->toContain('refresh_token_1234');
    expect((string) $rawStored)->not->toContain('ya29.access-token');
});

test('disconnecting google clears secret and marks integration inactive', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'google',
        'status' => 'active',
        'encrypted_secret' => json_encode([
            'refresh_token' => '1//refresh_token_1234',
            'access_token' => 'ya29.access-token',
            'integration_id' => 'gmail.primary',
        ], JSON_THROW_ON_ERROR),
        'secret_last4' => '1234',
        'secret_fingerprint' => hash('sha256', '1//refresh_token_1234'),
        'connected_at' => now(),
    ]);

    $response = $this
        ->actingAs($user)
        ->delete(route('settings.integrations.google.disconnect'));

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect(route('settings.integrations.edit'));

    $integration = UserIntegration::query()
        ->where('user_id', $user->id)
        ->where('provider', 'google')
        ->first();

    expect((string) $integration->status)->toBe('inactive');
    expect($integration->encrypted_secret)->toBeNull();
    expect($integration->secret_last4)->toBeNull();
});
