<?php

use App\Models\User;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;

test('integration secret lifecycle create rotate revoke is audited', function () {
    $user = User::factory()->create();

    $firstKey = 're_first_1234567890';
    $secondKey = 're_second_0987654321';

    $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.upsert'), [
            'resend_api_key' => $firstKey,
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ])
        ->assertSessionHasNoErrors();

    $integration = UserIntegration::query()->where('user_id', $user->id)->where('provider', 'resend')->first();

    expect($integration)->not->toBeNull();
    expect((string) $integration->status)->toBe('active');
    expect((string) $integration->secret_last4)->toBe('7890');

    $firstRawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $firstRawStored)->not->toBe($firstKey);

    $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.upsert'), [
            'resend_api_key' => $secondKey,
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ])
        ->assertSessionHasNoErrors();

    $integration->refresh();

    expect((string) $integration->status)->toBe('active');
    expect((string) $integration->secret_last4)->toBe('4321');

    $secondRawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $secondRawStored)->not->toBe($secondKey);
    expect((string) $secondRawStored)->not->toBe((string) $firstRawStored);

    $this
        ->actingAs($user)
        ->delete(route('settings.integrations.resend.disconnect'))
        ->assertSessionHasNoErrors();

    $integration->refresh();

    expect((string) $integration->status)->toBe('inactive');
    expect($integration->encrypted_secret)->toBeNull();

    $actions = UserIntegrationAudit::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->orderBy('id')
        ->pluck('action')
        ->all();

    expect($actions)->toContain('secret_created');
    expect($actions)->toContain('secret_rotated');
    expect($actions)->toContain('secret_revoked');

    $auditMetadataRows = UserIntegrationAudit::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->pluck('metadata')
        ->all();

    $encoded = json_encode($auditMetadataRows);
    expect($encoded)->not->toContain($firstKey);
    expect($encoded)->not->toContain($secondKey);
});

test('test resend endpoint writes audit entry', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_test_1234567890',
        'secret_fingerprint' => hash('sha256', 're_test_1234567890'),
        'secret_last4' => '7890',
        'connected_at' => now(),
    ]);

    $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.test'))
        ->assertSessionHasNoErrors();

    $audit = UserIntegrationAudit::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->where('action', 'test_requested')
        ->latest('id')
        ->first();

    expect($audit)->not->toBeNull();
    expect((string) ($audit->metadata['secret_last4'] ?? ''))->toBe('7890');
});

test('google oauth lifecycle create rotate revoke is audited', function () {
    $user = User::factory()->create();

    config()->set('services.google.client_id', 'google-client-id');
    config()->set('services.google.client_secret', 'google-client-secret');
    config()->set('services.google.redirect_uri', 'https://openagents.test/settings/integrations/google/callback');

    Http::fake([
        'https://oauth2.googleapis.com/token' => Http::sequence()
            ->push([
                'access_token' => 'ya29.first-access',
                'refresh_token' => '1//refresh_google_1111',
                'scope' => 'https://www.googleapis.com/auth/gmail.readonly',
                'token_type' => 'Bearer',
                'expires_in' => 3600,
            ], 200)
            ->push([
                'access_token' => 'ya29.second-access',
                'refresh_token' => '1//refresh_google_2222',
                'scope' => 'https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send',
                'token_type' => 'Bearer',
                'expires_in' => 7200,
            ], 200),
    ]);

    $this
        ->actingAs($user)
        ->withSession([
            'settings.integrations.google_oauth_state' => 'google-state-1',
        ])
        ->get(route('settings.integrations.google.callback', [
            'code' => 'google-code-1',
            'state' => 'google-state-1',
        ]))
        ->assertSessionHasNoErrors();

    $integration = UserIntegration::query()->where('user_id', $user->id)->where('provider', 'google')->first();
    expect($integration)->not->toBeNull();
    expect((string) $integration->secret_last4)->toBe('1111');

    $firstRawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $firstRawStored)->not->toContain('refresh_google_1111');

    $this
        ->actingAs($user)
        ->withSession([
            'settings.integrations.google_oauth_state' => 'google-state-2',
        ])
        ->get(route('settings.integrations.google.callback', [
            'code' => 'google-code-2',
            'state' => 'google-state-2',
        ]))
        ->assertSessionHasNoErrors();

    $integration->refresh();
    expect((string) $integration->secret_last4)->toBe('2222');

    $secondRawStored = DB::table('user_integrations')->where('id', $integration->id)->value('encrypted_secret');
    expect((string) $secondRawStored)->not->toContain('refresh_google_2222');
    expect((string) $secondRawStored)->not->toBe((string) $firstRawStored);

    $this
        ->actingAs($user)
        ->delete(route('settings.integrations.google.disconnect'))
        ->assertSessionHasNoErrors();

    $integration->refresh();
    expect((string) $integration->status)->toBe('inactive');
    expect($integration->encrypted_secret)->toBeNull();

    $actions = UserIntegrationAudit::query()
        ->where('user_id', $user->id)
        ->where('provider', 'google')
        ->orderBy('id')
        ->pluck('action')
        ->all();

    expect($actions)->toContain('secret_created');
    expect($actions)->toContain('secret_rotated');
    expect($actions)->toContain('secret_revoked');
});
