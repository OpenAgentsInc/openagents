<?php

use Illuminate\Support\Facades\File;

test('ops:capture-parity-contract-fixtures writes golden http and khala fixtures', function () {
    $outputDir = storage_path('framework/testing/parity-contract-fixtures-'.uniqid('', true));
    $openapiTempPath = storage_path('framework/testing/openapi-parity-capture-'.uniqid('', true).'.json');

    File::deleteDirectory($outputDir);
    File::delete($openapiTempPath);

    $this->artisan('ops:capture-parity-contract-fixtures', [
        '--output' => $outputDir,
        '--openapi-temp' => $openapiTempPath,
    ])
        ->expectsOutputToContain('Parity contract fixtures captured.')
        ->assertExitCode(0);

    $httpPath = $outputDir.'/http-json-golden.json';
    $khalaPath = $outputDir.'/khala-ws-golden.json';
    $indexPath = $outputDir.'/capture-index.json';

    expect(File::exists($httpPath))->toBeTrue();
    expect(File::exists($khalaPath))->toBeTrue();
    expect(File::exists($indexPath))->toBeTrue();

    $httpGolden = json_decode((string) File::get($httpPath), true, flags: JSON_THROW_ON_ERROR);
    $khalaGolden = json_decode((string) File::get($khalaPath), true, flags: JSON_THROW_ON_ERROR);
    $captureIndex = json_decode((string) File::get($indexPath), true, flags: JSON_THROW_ON_ERROR);

    expect($httpGolden['schema'])->toBe('openagents.webparity.http_golden.v1');
    expect($khalaGolden['schema'])->toBe('openagents.webparity.khala_ws_golden.v1');
    expect($captureIndex['artifacts'])->toHaveCount(2);

    expect(collect($httpGolden['fixtures'])->contains(
        fn (array $fixture): bool => $fixture['method'] === 'POST' && $fixture['path'] === '/api/auth/register'
    ))->toBeTrue();
    expect(collect($khalaGolden['frames'])->contains(
        fn (array $fixture): bool => ($fixture['name'] ?? null) === 'replay_batch'
    ))->toBeTrue();
    expect(($khalaGolden['replay_event_count'] ?? 0) > 0)->toBeTrue();

    File::deleteDirectory($outputDir);
    File::delete($openapiTempPath);
});
