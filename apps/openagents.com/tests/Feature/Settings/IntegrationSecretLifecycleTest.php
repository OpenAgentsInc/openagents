<?php

use App\Models\User;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use Illuminate\Support\Facades\DB;

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
