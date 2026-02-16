<?php

use App\AI\Tools\LightningL402FetchTool;
use Laravel\Ai\Tools\Request;

test('fetch tool resolves endpointPreset defaults for url method body and scope', function () {
    config()->set('lightning.demo_presets.preset_test', [
        'url' => 'https://sats4ai.com/api/l402/text-generation',
        'method' => 'POST',
        'headers' => ['Content-Type' => 'application/json'],
        'body' => '{"input":[{"role":"User","content":"hello"}],"model":"Best"}',
        'scope' => 'ep212.sats4ai',
    ]);

    $tool = new LightningL402FetchTool;

    $json = $tool->handle(new Request([
        'endpointPreset' => 'preset_test',
        'maxSpendSats' => 100,
        'approvalRequired' => true,
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('approval_requested');
    expect($result['host'])->toBe('sats4ai.com');
    expect($result['method'])->toBe('POST');
    expect($result['scope'])->toBe('ep212.sats4ai');
    expect($result['taskId'])->toBeString();
});
