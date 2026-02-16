<?php

use App\Models\User;
use Illuminate\Support\Facades\URL;

function signedTestLoginUrl(string $email, array $query = []): string
{
    return URL::temporarySignedRoute(
        'internal.test-login',
        now()->addMinutes(10),
        array_merge(['email' => $email], $query),
    );
}

test('maintenance test-login route returns not found when disabled', function () {
    config()->set('auth.local_test_login.enabled', false);
    config()->set('auth.local_test_login.allowed_emails', ['tester@openagents.com']);

    $response = $this->get(signedTestLoginUrl('tester@openagents.com'));

    $response->assertNotFound();
    $this->assertGuest();
});

test('maintenance test-login route requires signed url', function () {
    config()->set('auth.local_test_login.enabled', true);
    config()->set('auth.local_test_login.allowed_emails', ['tester@openagents.com']);

    $response = $this->get('/internal/test-login?email=tester@openagents.com');

    $response->assertForbidden();
    $this->assertGuest();
});

test('maintenance test-login route enforces allowlisted email', function () {
    config()->set('auth.local_test_login.enabled', true);
    config()->set('auth.local_test_login.allowed_emails', ['chris@openagents.com']);

    $response = $this->get(signedTestLoginUrl('tester@openagents.com'));

    $response->assertForbidden();
    $this->assertGuest();
});

test('maintenance test-login creates allowlisted user and can access chat', function () {
    config()->set('auth.local_test_login.enabled', true);
    config()->set('auth.local_test_login.allowed_emails', ['tester@openagents.com']);

    $response = $this->get(signedTestLoginUrl('tester@openagents.com', [
        'name' => 'Maintenance Tester',
    ]));

    $response->assertRedirect('/chat');
    $response->assertSessionHas('oa_local_test_auth', true);
    $this->assertAuthenticated();

    $user = User::query()->where('email', 'tester@openagents.com')->first();

    expect($user)->not->toBeNull();
    expect((string) $user->name)->toBe('Maintenance Tester');
    expect((string) $user->workos_id)->toStartWith('test_local_');

    $chatResponse = $this->get('/chat');
    $chatResponse->assertRedirect();
    expect((string) $chatResponse->headers->get('Location'))->toContain('/chat/');
});
