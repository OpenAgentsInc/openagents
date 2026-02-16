<?php

test('ops:test-login-link outputs a signed url', function () {
    config()->set('auth.local_test_login.enabled', true);

    $this->artisan('ops:test-login-link', [
        'email' => 'tester@openagents.com',
        '--minutes' => 15,
        '--base-url' => 'https://next.openagents.com',
    ])
        ->expectsOutputToContain('Signed maintenance test-login URL:')
        ->expectsOutputToContain('https://next.openagents.com/internal/test-login?')
        ->assertExitCode(0);
});

test('ops:test-login-link validates email argument', function () {
    $this->artisan('ops:test-login-link', [
        'email' => 'not-an-email',
    ])
        ->expectsOutputToContain('Invalid email address.')
        ->assertExitCode(1);
});
