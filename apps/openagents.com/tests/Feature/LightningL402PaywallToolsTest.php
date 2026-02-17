<?php

use App\AI\Tools\LightningL402PaywallCreateTool;
use App\AI\Tools\LightningL402PaywallDeleteTool;
use App\AI\Tools\LightningL402PaywallUpdateTool;
use App\Models\L402Paywall;
use App\Models\User;
use Laravel\Ai\Tools\Request;

beforeEach(function () {
    config()->set('admin.emails', ['chris@openagents.com']);
    config()->set('lightning.operator.aperture_config_path', storage_path('app/testing/l402-paywall-tools.json'));

    $path = (string) config('lightning.operator.aperture_config_path');
    if (is_file($path)) {
        unlink($path);
    }
});

test('paywall tools create update and delete with deterministic operation and deployment references', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 0"');

    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $this->actingAs($admin);

    $createTool = new LightningL402PaywallCreateTool;
    $updateTool = new LightningL402PaywallUpdateTool;
    $deleteTool = new LightningL402PaywallDeleteTool;

    $created = json_decode($createTool->handle(new Request([
        'name' => 'EP212 Seller Route',
        'hostRegexp' => '^l402\\.openagents\\.com$',
        'pathRegexp' => '^/ep212/seller$',
        'priceMsats' => 42000,
        'upstream' => 'https://example.com/seller',
        'enabled' => true,
    ])), true);

    expect($created)->toBeArray();
    expect($created['status'])->toBe('completed');
    expect($created['toolName'])->toBe('lightning_l402_paywall_create');
    expect($created['mutationEventId'])->toBeInt();
    expect($created['deploymentEventId'])->toBeInt();
    expect($created['operationId'])->toBe('l402_paywall_created:'.$created['mutationEventId']);

    $paywallId = (string) $created['paywall']['id'];
    expect($paywallId)->not->toBe('');
    expect(L402Paywall::query()->where('id', $paywallId)->exists())->toBeTrue();

    $updated = json_decode($updateTool->handle(new Request([
        'paywallId' => $paywallId,
        'priceMsats' => 50000,
        'upstream' => 'https://example.com/seller-v2',
    ])), true);

    expect($updated)->toBeArray();
    expect($updated['status'])->toBe('completed');
    expect($updated['toolName'])->toBe('lightning_l402_paywall_update');
    expect($updated['mutationEventId'])->toBeInt();
    expect($updated['deploymentEventId'])->toBeInt();
    expect($updated['operationId'])->toBe('l402_paywall_updated:'.$updated['mutationEventId']);
    expect((int) $updated['paywall']['priceMsats'])->toBe(50000);

    $deleted = json_decode($deleteTool->handle(new Request([
        'paywallId' => $paywallId,
    ])), true);

    expect($deleted)->toBeArray();
    expect($deleted['status'])->toBe('completed');
    expect($deleted['toolName'])->toBe('lightning_l402_paywall_delete');
    expect($deleted['mutationEventId'])->toBeInt();
    expect($deleted['deploymentEventId'])->toBeInt();
    expect($deleted['operationId'])->toBe('l402_paywall_deleted:'.$deleted['mutationEventId']);
    expect((string) $deleted['paywall']['id'])->toBe($paywallId);
    expect($deleted['paywall']['deletedAt'])->not->toBeNull();

    expect(L402Paywall::query()->where('id', $paywallId)->exists())->toBeFalse();
    expect(L402Paywall::withTrashed()->where('id', $paywallId)->exists())->toBeTrue();
});

test('paywall tools reject unauthorized users', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 0"');

    $owner = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $paywall = L402Paywall::query()->create([
        'owner_user_id' => $owner->id,
        'name' => 'Owned Route',
        'host_regexp' => '^l402\\.openagents\\.com$',
        'path_regexp' => '^/owned$',
        'price_msats' => 1000,
        'upstream' => 'https://example.com/owned',
        'enabled' => true,
    ]);

    $nonAdmin = User::factory()->create([
        'email' => 'dev@openagents.com',
    ]);

    $this->actingAs($nonAdmin);

    $create = json_decode((new LightningL402PaywallCreateTool)->handle(new Request([
        'name' => 'Forbidden',
        'hostRegexp' => '^l402\\.openagents\\.com$',
        'pathRegexp' => '^/forbidden$',
        'priceMsats' => 1000,
        'upstream' => 'https://example.com/forbidden',
    ])), true);

    $update = json_decode((new LightningL402PaywallUpdateTool)->handle(new Request([
        'paywallId' => (string) $paywall->id,
        'priceMsats' => 2000,
    ])), true);

    $delete = json_decode((new LightningL402PaywallDeleteTool)->handle(new Request([
        'paywallId' => (string) $paywall->id,
    ])), true);

    expect($create['status'])->toBe('failed');
    expect($create['denyCode'])->toBe('operator_forbidden');
    expect($update['status'])->toBe('failed');
    expect($update['denyCode'])->toBe('operator_forbidden');
    expect($delete['status'])->toBe('failed');
    expect($delete['denyCode'])->toBe('operator_forbidden');

    auth()->logout();

    $guest = json_decode((new LightningL402PaywallCreateTool)->handle(new Request([
        'name' => 'Guest Forbidden',
        'hostRegexp' => '^l402\\.openagents\\.com$',
        'pathRegexp' => '^/guest$',
        'priceMsats' => 1000,
        'upstream' => 'https://example.com/guest',
    ])), true);

    expect($guest['status'])->toBe('failed');
    expect($guest['denyCode'])->toBe('operator_forbidden');
});

test('paywall tools enforce host path and price guardrails', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 0"');

    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $this->actingAs($admin);

    $createTool = new LightningL402PaywallCreateTool;
    $updateTool = new LightningL402PaywallUpdateTool;

    $blockedCreate = json_decode($createTool->handle(new Request([
        'name' => 'Too broad',
        'hostRegexp' => '.*',
        'pathRegexp' => '^/.*$',
        'priceMsats' => 0,
        'upstream' => 'https://example.com/broad',
    ])), true);

    expect($blockedCreate['status'])->toBe('blocked');
    expect($blockedCreate['denyCode'])->toBe('validation_failed');
    expect($blockedCreate['errors'])->toHaveKey('hostRegexp');
    expect($blockedCreate['errors'])->toHaveKey('pathRegexp');
    expect($blockedCreate['errors'])->toHaveKey('priceMsats');

    $created = json_decode($createTool->handle(new Request([
        'name' => 'Valid',
        'hostRegexp' => '^l402\\.openagents\\.com$',
        'pathRegexp' => '^/valid$',
        'priceMsats' => 1000,
        'upstream' => 'https://example.com/valid',
    ])), true);

    $paywallId = (string) $created['paywall']['id'];

    $blockedUpdateEmpty = json_decode($updateTool->handle(new Request([
        'paywallId' => $paywallId,
    ])), true);

    expect($blockedUpdateEmpty['status'])->toBe('blocked');
    expect($blockedUpdateEmpty['denyCode'])->toBe('validation_failed');
    expect($blockedUpdateEmpty['errors'])->toHaveKey('payload');

    $blockedUpdatePath = json_decode($updateTool->handle(new Request([
        'paywallId' => $paywallId,
        'pathRegexp' => '^/.*$',
    ])), true);

    expect($blockedUpdatePath['status'])->toBe('blocked');
    expect($blockedUpdatePath['denyCode'])->toBe('validation_failed');
    expect($blockedUpdatePath['errors'])->toHaveKey('pathRegexp');
});

test('paywall create tool returns reconcile failure context and reverts changes on failed deploy', function () {
    config()->set('lightning.operator.aperture_reconcile_command', 'sh -c "exit 1"');

    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $this->actingAs($admin);

    $result = json_decode((new LightningL402PaywallCreateTool)->handle(new Request([
        'name' => 'Will fail',
        'hostRegexp' => '^l402\\.openagents\\.com$',
        'pathRegexp' => '^/will-fail$',
        'priceMsats' => 5000,
        'upstream' => 'https://example.com/will-fail',
    ])), true);

    expect($result['status'])->toBe('failed');
    expect($result['denyCode'])->toBe('reconcile_failed');
    expect($result['reverted'])->toBeTrue();
    expect($result['context'])->toBeArray();
    expect($result['context']['action'] ?? null)->toBe('create');
    expect(L402Paywall::query()->count())->toBe(0);
});
