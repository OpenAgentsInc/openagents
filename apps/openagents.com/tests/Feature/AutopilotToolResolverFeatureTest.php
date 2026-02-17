<?php

use App\AI\Runtime\AutopilotExecutionContext;
use App\AI\Tools\AutopilotToolResolver;
use App\Models\AutopilotPolicy;
use Illuminate\Support\Str;

test('resolver applies autopilot policy allowlist and denylist to exposed tools', function () {
    $autopilotId = (string) Str::uuid7();

    AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilotId,
        'tool_allowlist' => ['openagents_api', 'lightning_l402_fetch'],
        'tool_denylist' => ['lightning_l402_fetch'],
    ]);

    $context = resolve(AutopilotExecutionContext::class);
    $context->set(1, $autopilotId, true);

    $resolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot($autopilotId);

    $toolNames = array_map(fn ($tool) => $tool->name(), $resolution['tools']);

    expect($toolNames)->toBe(['openagents_api']);
    expect($resolution['audit']['policyApplied'] ?? null)->toBeTrue();
    expect($resolution['audit']['authRestricted'] ?? null)->toBeFalse();
    expect($resolution['audit']['removedByDenylist'] ?? [])->toContain('lightning_l402_fetch');
    expect($resolution['audit']['removedByAllowlist'] ?? [])->toContain('lightning_l402_approve');
    expect($resolution['audit']['removedByAllowlist'] ?? [])->not->toContain('chat_login');
});

test('resolver falls back to authenticated tool registry when no autopilot policy exists', function () {
    $context = resolve(AutopilotExecutionContext::class);
    $context->set(1, null, true);

    $resolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot((string) Str::uuid7());

    $toolNames = array_map(fn ($tool) => $tool->name(), $resolution['tools']);

    expect($resolution['audit']['policyApplied'] ?? null)->toBeFalse();
    expect($resolution['audit']['authRestricted'] ?? null)->toBeFalse();
    expect($toolNames)->not->toContain('chat_login');
    expect($toolNames)->toContain('openagents_api');
    expect($toolNames)->toContain('lightning_l402_fetch');
    expect($toolNames)->toContain('lightning_l402_approve');
    expect($toolNames)->toContain('lightning_l402_paywall_create');
    expect($toolNames)->toContain('lightning_l402_paywall_update');
    expect($toolNames)->toContain('lightning_l402_paywall_delete');
    expect($toolNames)->not->toContain('get_time');
    expect($toolNames)->not->toContain('echo');
});

test('resolver exposes guest-safe tools for unauthenticated guest sessions', function () {
    $autopilotId = (string) Str::uuid7();

    AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilotId,
        'tool_allowlist' => ['openagents_api', 'lightning_l402_fetch'],
        'tool_denylist' => ['lightning_l402_fetch'],
    ]);

    $context = resolve(AutopilotExecutionContext::class);
    $context->set(null, $autopilotId, false);

    $resolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot($autopilotId);

    $toolNames = array_map(fn ($tool) => $tool->name(), $resolution['tools']);

    expect($toolNames)->toBe(['chat_login', 'openagents_api']);
    expect($resolution['audit']['policyApplied'] ?? null)->toBeFalse();
    expect($resolution['audit']['authRestricted'] ?? null)->toBeTrue();
    expect($resolution['audit']['sessionAuthenticated'] ?? null)->toBeFalse();
    expect($resolution['audit']['removedByAuthGate'] ?? [])->toContain('lightning_l402_fetch');
    expect($resolution['audit']['removedByAuthGate'] ?? [])->toContain('lightning_l402_approve');
});
