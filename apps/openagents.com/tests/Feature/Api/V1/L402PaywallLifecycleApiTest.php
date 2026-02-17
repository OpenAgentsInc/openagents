<?php

use App\Models\L402Paywall;
use App\Models\User;
use Illuminate\Support\Facades\DB;

beforeEach(function () {
    config()->set('admin.emails', ['chris@openagents.com']);
    config()->set('lightning.operator.aperture_config_path', storage_path('app/testing/l402-aperture-paywalls.json'));

    $path = (string) config('lightning.operator.aperture_config_path');
    if (is_file($path)) {
        unlink($path);
    }
});

it('creates updates and deletes paywalls through admin mutation api and emits deployment receipts', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 0"');

    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $token = $admin->createToken('admin-l402-paywalls')->plainTextToken;

    $created = $this->withToken($token)
        ->postJson('/api/l402/paywalls', [
            'name' => 'EP212 Premium',
            'hostRegexp' => '^l402\\.openagents\\.com$',
            'pathRegexp' => '^/ep212/premium-signal$',
            'priceMsats' => 42000,
            'upstream' => 'https://example.com/premium',
            'enabled' => true,
            'metadata' => ['tier' => 'premium'],
        ])
        ->assertCreated()
        ->assertJsonPath('data.paywall.name', 'EP212 Premium')
        ->assertJsonPath('data.paywall.priceMsats', 42000)
        ->assertJsonPath('data.deployment.status', 'succeeded');

    $paywallId = (string) $created->json('data.paywall.id');
    expect($paywallId)->not->toBe('');

    $this->withToken($token)
        ->patchJson('/api/l402/paywalls/'.$paywallId, [
            'priceMsats' => 50000,
            'upstream' => 'https://example.com/premium-v2',
        ])
        ->assertOk()
        ->assertJsonPath('data.paywall.priceMsats', 50000)
        ->assertJsonPath('data.paywall.upstream', 'https://example.com/premium-v2')
        ->assertJsonPath('data.deployment.status', 'succeeded');

    $this->withToken($token)
        ->deleteJson('/api/l402/paywalls/'.$paywallId)
        ->assertOk()
        ->assertJsonPath('data.deleted', true)
        ->assertJsonPath('data.paywall.id', $paywallId)
        ->assertJsonPath('data.deployment.status', 'succeeded');

    expect(L402Paywall::query()->where('id', $paywallId)->exists())->toBeFalse();
    expect(L402Paywall::withTrashed()->where('id', $paywallId)->exists())->toBeTrue();

    $deployments = $this->withToken($token)
        ->getJson('/api/l402/deployments')
        ->assertOk();

    $types = collect($deployments->json('data.deployments', []))->pluck('type')->values();
    expect($types)->toContain('l402_paywall_created');
    expect($types)->toContain('l402_paywall_updated');
    expect($types)->toContain('l402_paywall_deleted');
    expect($types)->toContain('l402_gateway_deployment');

    $snapshotPath = (string) config('lightning.operator.aperture_config_path');
    expect(is_file($snapshotPath))->toBeTrue();

    $snapshot = json_decode((string) file_get_contents($snapshotPath), true);
    expect($snapshot)->toBeArray();
    expect($snapshot['services'] ?? null)->toBeArray();
    expect($snapshot['services'])->toHaveCount(0);
});

it('forbids non-admin users from l402 paywall mutation endpoints', function () {
    $user = User::factory()->create([
        'email' => 'not-admin@openagents.com',
    ]);

    $token = $user->createToken('non-admin-l402-paywalls')->plainTextToken;

    $this->withToken($token)
        ->postJson('/api/l402/paywalls', [
            'name' => 'Forbidden',
            'hostRegexp' => '^l402\\.openagents\\.com$',
            'pathRegexp' => '^/ep212/forbidden$',
            'priceMsats' => 1000,
            'upstream' => 'https://example.com/forbidden',
        ])
        ->assertForbidden();

    $this->withToken($token)
        ->patchJson('/api/l402/paywalls/019c0000-0000-7000-8000-000000000000', [
            'priceMsats' => 2000,
        ])
        ->assertForbidden();

    $this->withToken($token)
        ->deleteJson('/api/l402/paywalls/019c0000-0000-7000-8000-000000000000')
        ->assertForbidden();
});

it('reverts paywall mutations when reconcile fails', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 1"');

    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $token = $admin->createToken('admin-l402-reconcile-fail')->plainTextToken;

    $this->withToken($token)
        ->postJson('/api/l402/paywalls', [
            'name' => 'Will Fail',
            'hostRegexp' => '^l402\\.openagents\\.com$',
            'pathRegexp' => '^/ep212/fail-create$',
            'priceMsats' => 9000,
            'upstream' => 'https://example.com/fail-create',
        ])
        ->assertStatus(422)
        ->assertJsonPath('errorCode', 'l402_reconcile_failed')
        ->assertJsonPath('reverted', true);

    expect(L402Paywall::query()->count())->toBe(0);

    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 0"');

    $created = $this->withToken($token)
        ->postJson('/api/l402/paywalls', [
            'name' => 'Stable',
            'hostRegexp' => '^l402\\.openagents\\.com$',
            'pathRegexp' => '^/ep212/stable$',
            'priceMsats' => 15000,
            'upstream' => 'https://example.com/stable',
        ])
        ->assertCreated();

    $paywallId = (string) $created->json('data.paywall.id');

    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 1"');

    $this->withToken($token)
        ->patchJson('/api/l402/paywalls/'.$paywallId, [
            'priceMsats' => 20000,
        ])
        ->assertStatus(422)
        ->assertJsonPath('reverted', true);

    $paywallAfterFailedUpdate = L402Paywall::query()->findOrFail($paywallId);
    expect((int) $paywallAfterFailedUpdate->price_msats)->toBe(15000);

    $this->withToken($token)
        ->deleteJson('/api/l402/paywalls/'.$paywallId)
        ->assertStatus(422)
        ->assertJsonPath('reverted', true);

    expect(L402Paywall::query()->where('id', $paywallId)->exists())->toBeTrue();

    $mutationTypes = DB::table('run_events')
        ->where('user_id', $admin->id)
        ->whereIn('type', ['l402_paywall_created', 'l402_paywall_updated', 'l402_paywall_deleted'])
        ->pluck('type')
        ->all();

    expect($mutationTypes)->toContain('l402_paywall_created');
    expect($mutationTypes)->toContain('l402_paywall_updated');
    expect($mutationTypes)->toContain('l402_paywall_deleted');
});
