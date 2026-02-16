<?php

test('demo:l402 command succeeds deterministically against the in-process fake preset', function () {
    $expectedSha = hash('sha256', 'demo premium payload');

    $this->artisan('demo:l402', [
        '--preset' => 'fake',
        '--max-spend-sats' => 100,
    ])
        ->expectsOutputToContain('L402 demo result')
        ->expectsOutputToContain('preset: fake')
        ->expectsOutputToContain('status: completed')
        ->expectsOutputToContain('response_sha256: '.$expectedSha)
        ->assertExitCode(0);
});
