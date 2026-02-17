<?php

use App\AI\Tools\AutopilotToolResolver;

test('resolveToolNames applies allowlist-only mode deterministically', function () {
    $resolved = AutopilotToolResolver::resolveToolNames(
        availableToolNames: ['get_time', 'echo', 'lightning_l402_fetch', 'lightning_l402_approve'],
        allowlist: ['echo', 'get_time'],
        denylist: [],
    );

    expect($resolved['exposed'])->toBe(['get_time', 'echo']);
    expect($resolved['removedByAllowlist'])->toBe(['lightning_l402_fetch', 'lightning_l402_approve']);
    expect($resolved['removedByDenylist'])->toBe([]);
});

test('resolveToolNames applies denylist after allowlist and keeps order stable', function () {
    $resolved = AutopilotToolResolver::resolveToolNames(
        availableToolNames: ['get_time', 'echo', 'lightning_l402_fetch', 'lightning_l402_approve'],
        allowlist: ['echo', 'get_time', 'lightning_l402_fetch'],
        denylist: ['get_time', 'lightning_l402_fetch'],
    );

    expect($resolved['exposed'])->toBe(['echo']);
    expect($resolved['removedByAllowlist'])->toBe(['lightning_l402_approve']);
    expect($resolved['removedByDenylist'])->toBe(['get_time', 'lightning_l402_fetch']);
});
