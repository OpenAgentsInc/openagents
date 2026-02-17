<?php

it('generates OpenAPI spec with api coverage and sanctum security', function () {
    $outputPath = storage_path('app/openapi-test-output.json');

    if (file_exists($outputPath)) {
        unlink($outputPath);
    }

    $this->artisan('openapi:generate', ['--output' => $outputPath])
        ->assertExitCode(0);

    expect(file_exists($outputPath))->toBeTrue();

    $raw = file_get_contents($outputPath);
    expect($raw)->not->toBeFalse();

    $spec = json_decode((string) $raw, true);

    expect($spec)->toBeArray();
    expect($spec['openapi'] ?? null)->toBe('3.0.2');

    $paths = $spec['paths'] ?? [];

    expect($paths)->toHaveKey('/api/me');
    expect($paths)->toHaveKey('/api/tokens');
    expect($paths)->toHaveKey('/api/chats/{conversationId}/stream');
    expect($paths)->toHaveKey('/api/chat/stream');
    expect($paths)->toHaveKey('/api/autopilots');
    expect($paths)->toHaveKey('/api/autopilots/{autopilot}');
    expect($paths)->toHaveKey('/api/autopilots/{autopilot}/threads');
    expect($paths)->toHaveKey('/api/autopilots/{autopilot}/stream');
    expect($paths)->toHaveKey('/api/l402/wallet');
    expect($paths)->toHaveKey('/api/agent-payments/wallet');
    expect($paths)->toHaveKey('/api/payments/pay');
    expect($paths)->toHaveKey('/api/shouts');
    expect($paths)->toHaveKey('/api/shouts/zones');
    expect($paths)->toHaveKey('/api/whispers');
    expect($paths)->toHaveKey('/api/whispers/{id}/read');

    expect($paths)->not->toHaveKey('/api/chat');
    expect($paths)->not->toHaveKey('/api/admin/status');
    expect($paths)->not->toHaveKey('/api/v1/me');

    expect(data_get($spec, 'components.securitySchemes.SanctumToken.type'))->toBe('http');
    expect(data_get($spec, 'components.securitySchemes.SanctumToken.scheme'))->toBe('bearer');

    @unlink($outputPath);
});

it('keeps committed openapi json in parity with generated output', function () {
    $generatedPath = storage_path('app/openapi-parity-output.json');

    if (file_exists($generatedPath)) {
        unlink($generatedPath);
    }

    $this->artisan('openapi:generate', ['--output' => $generatedPath])
        ->assertExitCode(0);

    $generatedRaw = file_get_contents($generatedPath);
    $committedRaw = file_get_contents(base_path('public/openapi.json'));

    expect($generatedRaw)->not->toBeFalse();
    expect($committedRaw)->not->toBeFalse();

    $generated = json_decode((string) $generatedRaw, true);
    $committed = json_decode((string) $committedRaw, true);

    expect($generated)->toBeArray();
    expect($committed)->toBeArray();

    $canonicalize = function ($value) use (&$canonicalize) {
        if (! is_array($value)) {
            return $value;
        }

        $normalized = array_map(fn ($child) => $canonicalize($child), $value);
        $keys = array_keys($normalized);
        $isList = $keys === range(0, count($normalized) - 1);

        if (! $isList) {
            ksort($normalized);
        }

        return $normalized;
    };

    expect($canonicalize($generated))->toBe($canonicalize($committed));

    @unlink($generatedPath);
});

it('serves OpenAPI spec from openapi json route', function () {
    $response = $this->get('/openapi.json');

    $response->assertOk();

    $spec = $response->json();
    expect($spec)->toBeArray();
    expect($spec['openapi'] ?? null)->toBe('3.0.2');

    $paths = $spec['paths'] ?? [];
    expect($paths)->toHaveKey('/api/me');
    expect($paths)->toHaveKey('/api/settings/profile');
    expect($paths)->toHaveKey('/api/autopilots');
    expect($paths)->toHaveKey('/api/autopilots/{autopilot}/stream');
    expect($paths)->toHaveKey('/api/shouts');
    expect($paths)->toHaveKey('/api/whispers');
    expect($paths)->not->toHaveKey('/api/v1/me');
});

it('returns minified JSON at /openapi.json', function () {
    $response = $this->get('/openapi.json');

    $response->assertOk();

    $content = (string) $response->getContent();

    expect($content)->toStartWith('{"openapi":');
    expect($content)->not->toContain("\n");
});
