<?php

use App\Models\L402Paywall;
use App\Models\User;
use Illuminate\Testing\TestResponse;

beforeEach(function () {
    config()->set('lightning.operator.ops_secret', 'test-ops-secret');
});

test('lightning ops control-plane API rejects invalid secret', function () {
    $response = $this->postJson('/api/internal/lightning-ops/control-plane/query', [
        'functionName' => 'lightning/ops:listPaywallControlPlaneState',
        'args' => [
            'secret' => 'wrong-secret',
            'statuses' => ['active', 'paused'],
        ],
    ]);

    $response
        ->assertStatus(401)
        ->assertJsonPath('error.code', 'invalid_ops_secret');
});

test('lightning ops control-plane API returns paywall snapshot with active and paused statuses', function () {
    $owner = User::factory()->create();

    $active = L402Paywall::query()->create([
        'owner_user_id' => $owner->id,
        'name' => 'Active Paywall',
        'host_regexp' => '^l402\\.openagents\\.com$',
        'path_regexp' => '^/active$',
        'price_msats' => 42000,
        'upstream' => 'https://example.com/active',
        'enabled' => true,
        'meta' => [
            'priority' => 12,
            'timeoutMs' => 7500,
        ],
    ]);

    $paused = L402Paywall::query()->create([
        'owner_user_id' => $owner->id,
        'name' => 'Paused Paywall',
        'host_regexp' => '^l402\\.openagents\\.com$',
        'path_regexp' => '^/paused$',
        'price_msats' => 21000,
        'upstream' => 'https://example.com/paused',
        'enabled' => false,
    ]);

    $archived = L402Paywall::query()->create([
        'owner_user_id' => $owner->id,
        'name' => 'Archived Paywall',
        'host_regexp' => '^l402\\.openagents\\.com$',
        'path_regexp' => '^/archived$',
        'price_msats' => 11000,
        'upstream' => 'https://example.com/archived',
        'enabled' => true,
    ]);
    $archived->delete();

    $response = ops_control_plane_query($this, 'lightning/ops:listPaywallControlPlaneState', [
        'statuses' => ['active', 'paused'],
    ]);

    $response
        ->assertOk()
        ->assertJsonPath('ok', true);

    $paywalls = collect($response->json('paywalls'));
    expect($paywalls)->toHaveCount(2);
    expect($paywalls->pluck('paywallId')->all())->toContain((string) $active->id);
    expect($paywalls->pluck('paywallId')->all())->toContain((string) $paused->id);
    expect($paywalls->pluck('paywallId')->all())->not->toContain((string) $archived->id);

    $activePayload = $paywalls->firstWhere('paywallId', (string) $active->id);
    expect($activePayload['status'])->toBe('active');
    expect($activePayload['policy']['pricingMode'])->toBe('fixed');
    expect($activePayload['routes'][0]['protocol'])->toBe('https');
    expect($activePayload['routes'][0]['priority'])->toBe(12);
    expect($activePayload['routes'][0]['timeoutMs'])->toBe(7500);
});

test('lightning ops control-plane API preserves deployment settlement and security parity semantics', function () {
    $deployment = ops_control_plane_mutation($this, 'lightning/ops:recordGatewayCompileIntent', [
        'configHash' => 'cfg_test_1',
        'status' => 'pending',
        'diagnostics' => [],
        'metadata' => ['executionPath' => 'hosted-node'],
        'requestId' => 'req_compile_1',
    ]);

    $deployment
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('deployment.status', 'pending')
        ->assertJsonPath('deployment.configHash', 'cfg_test_1');

    $deploymentId = (string) $deployment->json('deployment.deploymentId');
    expect($deploymentId)->not->toBe('');

    ops_control_plane_mutation($this, 'lightning/ops:recordGatewayDeploymentEvent', [
        'paywallId' => 'pw_1',
        'ownerId' => 'owner_1',
        'eventType' => 'deploy.apply',
        'level' => 'info',
        'requestId' => 'req_compile_1',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('event.level', 'info');

    ops_control_plane_mutation($this, 'lightning/settlements:ingestInvoiceLifecycle', [
        'invoiceId' => 'inv_1',
        'paywallId' => 'pw_1',
        'ownerId' => 'owner_1',
        'amountMsats' => 2500,
        'status' => 'open',
        'requestId' => 'req_settle_1',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('invoice.status', 'open');

    $firstSettlement = ops_control_plane_mutation($this, 'lightning/settlements:ingestSettlement', [
        'settlementId' => 'set_1',
        'paywallId' => 'pw_1',
        'ownerId' => 'owner_1',
        'invoiceId' => 'inv_1',
        'amountMsats' => 2500,
        'paymentProofType' => 'lightning_preimage',
        'paymentProofValue' => str_repeat('a', 64),
        'requestId' => 'req_settle_1',
    ]);

    $firstSettlement
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('existed', false)
        ->assertJsonPath('invoice.status', 'settled')
        ->assertJsonPath('settlement.paymentProofRef', 'lightning_preimage:aaaaaaaaaaaaaaaaaaaaaaaa');

    ops_control_plane_mutation($this, 'lightning/settlements:ingestInvoiceLifecycle', [
        'invoiceId' => 'inv_1',
        'paywallId' => 'pw_1',
        'ownerId' => 'owner_1',
        'amountMsats' => 2500,
        'status' => 'open',
        'requestId' => 'req_settle_2',
    ])
        ->assertOk()
        ->assertJsonPath('invoice.status', 'settled');

    ops_control_plane_mutation($this, 'lightning/settlements:ingestSettlement', [
        'settlementId' => 'set_1',
        'paywallId' => 'pw_1',
        'ownerId' => 'owner_1',
        'invoiceId' => 'inv_1',
        'amountMsats' => 2500,
        'paymentProofType' => 'lightning_preimage',
        'paymentProofValue' => str_repeat('a', 64),
        'requestId' => 'req_settle_1',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('existed', true);

    ops_control_plane_mutation($this, 'lightning/security:setGlobalPause', [
        'active' => true,
        'reason' => 'smoke global pause',
        'updatedBy' => 'ops-test',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('global.globalPause', true)
        ->assertJsonPath('global.denyReasonCode', 'global_pause_active');

    ops_control_plane_mutation($this, 'lightning/security:setOwnerKillSwitch', [
        'ownerId' => 'owner_1',
        'active' => true,
        'reason' => 'owner paused',
        'updatedBy' => 'ops-test',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('ownerControl.killSwitch', true)
        ->assertJsonPath('ownerControl.denyReasonCode', 'owner_kill_switch_active');

    $rotated = ops_control_plane_mutation($this, 'lightning/security:rotateCredentialRole', [
        'role' => 'gateway_invoice',
        'fingerprint' => 'fp_rotate',
        'note' => 'rotating',
    ]);
    $rotated
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('role.status', 'rotating');

    ops_control_plane_mutation($this, 'lightning/security:revokeCredentialRole', [
        'role' => 'gateway_invoice',
        'note' => 'revoked',
    ])
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('role.status', 'revoked');

    $activated = ops_control_plane_mutation($this, 'lightning/security:activateCredentialRole', [
        'role' => 'gateway_invoice',
        'fingerprint' => 'fp_active',
        'note' => 'active',
    ]);
    $activated
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('role.status', 'active');

    expect((int) $activated->json('role.version'))->toBe((int) $rotated->json('role.version') + 1);

    ops_control_plane_query($this, 'lightning/security:getControlPlaneSecurityState')
        ->assertOk()
        ->assertJsonPath('ok', true)
        ->assertJsonPath('global.globalPause', true);
});

/**
 * @param  array<string, mixed>  $args
 */
function ops_control_plane_query(\Tests\TestCase $testCase, string $functionName, array $args = []): TestResponse
{
    $secret = (string) config('lightning.operator.ops_secret', '');

    return $testCase->postJson('/api/internal/lightning-ops/control-plane/query', [
        'functionName' => $functionName,
        'args' => $args + ['secret' => $secret],
    ]);
}

/**
 * @param  array<string, mixed>  $args
 */
function ops_control_plane_mutation(\Tests\TestCase $testCase, string $functionName, array $args = []): TestResponse
{
    $secret = (string) config('lightning.operator.ops_secret', '');

    return $testCase->postJson('/api/internal/lightning-ops/control-plane/mutation', [
        'functionName' => $functionName,
        'args' => $args + ['secret' => $secret],
    ]);
}
