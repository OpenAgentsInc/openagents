<?php

use App\AI\Tools\AutopilotToolResolver;
use App\Models\AutopilotPolicy;
use Illuminate\Support\Str;

test('resolver applies autopilot policy allowlist and denylist to exposed tools', function () {
    $autopilotId = (string) Str::uuid7();

    AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilotId,
        'tool_allowlist' => ['echo', 'get_time'],
        'tool_denylist' => ['get_time'],
    ]);

    $resolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot($autopilotId);

    $toolNames = array_map(fn ($tool) => $tool->name(), $resolution['tools']);

    expect($toolNames)->toBe(['echo']);
    expect($resolution['audit']['policyApplied'] ?? null)->toBeTrue();
    expect($resolution['audit']['removedByDenylist'] ?? [])->toContain('get_time');
    expect($resolution['audit']['removedByAllowlist'] ?? [])->toContain('lightning_l402_fetch');
});

test('resolver falls back to full tool registry when no autopilot policy exists', function () {
    $resolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot((string) Str::uuid7());

    $toolNames = array_map(fn ($tool) => $tool->name(), $resolution['tools']);

    expect($resolution['audit']['policyApplied'] ?? null)->toBeFalse();
    expect($toolNames)->toContain('get_time');
    expect($toolNames)->toContain('echo');
    expect($toolNames)->toContain('lightning_l402_fetch');
    expect($toolNames)->toContain('lightning_l402_approve');
});
