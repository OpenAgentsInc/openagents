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
    expect($paths)->toHaveKey('/api/l402/wallet');
    expect($paths)->toHaveKey('/api/agent-payments/wallet');
    expect($paths)->toHaveKey('/api/payments/pay');

    expect($paths)->not->toHaveKey('/api/chat');

    expect(data_get($spec, 'components.securitySchemes.SanctumToken.type'))->toBe('http');
    expect(data_get($spec, 'components.securitySchemes.SanctumToken.scheme'))->toBe('bearer');

    @unlink($outputPath);
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
});

it('returns minified JSON at /openapi.json', function () {
    $response = $this->get('/openapi.json');

    $response->assertOk();

    $content = (string) $response->getContent();

    expect($content)->toStartWith('{"openapi":');
    expect($content)->not->toContain("\n");
});
