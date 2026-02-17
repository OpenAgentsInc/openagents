<?php

use App\AI\Tools\AutopilotToolResolver;

test('resolveToolNames applies allowlist-only mode deterministically', function () {
    $resolved = AutopilotToolResolver::resolveToolNames(
        availableToolNames: ['openagents_api', 'lightning_l402_fetch', 'lightning_l402_approve'],
        allowlist: ['openagents_api', 'lightning_l402_fetch'],
        denylist: [],
    );

    expect($resolved['exposed'])->toBe(['openagents_api', 'lightning_l402_fetch']);
    expect($resolved['removedByAllowlist'])->toBe(['lightning_l402_approve']);
    expect($resolved['removedByDenylist'])->toBe([]);
});

test('resolveToolNames applies denylist after allowlist and keeps order stable', function () {
    $resolved = AutopilotToolResolver::resolveToolNames(
        availableToolNames: ['openagents_api', 'lightning_l402_fetch', 'lightning_l402_approve', 'lightning_l402_paywall_create'],
        allowlist: ['openagents_api', 'lightning_l402_fetch', 'lightning_l402_approve'],
        denylist: ['lightning_l402_fetch'],
    );

    expect($resolved['exposed'])->toBe(['openagents_api', 'lightning_l402_approve']);
    expect($resolved['removedByAllowlist'])->toBe(['lightning_l402_paywall_create']);
    expect($resolved['removedByDenylist'])->toBe(['lightning_l402_fetch']);
});
