<?php

use Illuminate\Support\Facades\File;

test('ops:export-parity-manifests writes baseline JSON manifests', function () {
    $outputDir = storage_path('framework/testing/parity-manifests-'.uniqid('', true));
    File::deleteDirectory($outputDir);

    $this->artisan('ops:export-parity-manifests', [
        '--output' => $outputDir,
    ])
        ->expectsOutputToContain('Parity manifests exported.')
        ->assertExitCode(0);

    $apiManifestPath = $outputDir.'/api-routes.json';
    $webManifestPath = $outputDir.'/web-routes.json';
    $pagesManifestPath = $outputDir.'/page-entries.json';
    $commandsManifestPath = $outputDir.'/artisan-commands.json';
    $indexManifestPath = $outputDir.'/manifest-index.json';

    expect(File::exists($apiManifestPath))->toBeTrue();
    expect(File::exists($webManifestPath))->toBeTrue();
    expect(File::exists($pagesManifestPath))->toBeTrue();
    expect(File::exists($commandsManifestPath))->toBeTrue();
    expect(File::exists($indexManifestPath))->toBeTrue();

    $apiManifest = json_decode((string) File::get($apiManifestPath), true, flags: JSON_THROW_ON_ERROR);
    $webManifest = json_decode((string) File::get($webManifestPath), true, flags: JSON_THROW_ON_ERROR);
    $pagesManifest = json_decode((string) File::get($pagesManifestPath), true, flags: JSON_THROW_ON_ERROR);
    $commandsManifest = json_decode((string) File::get($commandsManifestPath), true, flags: JSON_THROW_ON_ERROR);
    $indexManifest = json_decode((string) File::get($indexManifestPath), true, flags: JSON_THROW_ON_ERROR);

    expect(collect($apiManifest['routes'])->contains(
        fn (array $route): bool => $route['method'] === 'GET' && $route['uri'] === '/api/me'
    ))->toBeTrue();
    expect(collect($webManifest['routes'])->contains(
        fn (array $route): bool => $route['method'] === 'GET' && $route['uri'] === '/login'
    ))->toBeTrue();
    expect($pagesManifest['entries'])->toContain('resources/js/pages/admin/index.tsx');
    expect(collect($commandsManifest['commands'])->contains(
        fn (array $command): bool => $command['name'] === 'demo:l402'
    ))->toBeTrue();
    expect(collect($commandsManifest['commands'])->contains(
        fn (array $command): bool => $command['name'] === 'ops:export-parity-manifests'
    ))->toBeTrue();
    expect($indexManifest['manifests'])->toHaveCount(4);

    File::deleteDirectory($outputDir);
});
