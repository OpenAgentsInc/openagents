<?php

use App\Models\User;

afterEach(function () {
    \Mockery::close();
});

function configureWorkosForTests(): void
{
    config()->set('services.workos.client_id', 'client_test_123');
    config()->set('services.workos.secret', 'sk_test_123');
    config()->set('services.workos.redirect_url', 'https://next.openagents.com/authenticate');
}

test('guests can view login page', function () {
    $this->get('/login')->assertOk();
});

test('register route redirects to login', function () {
    $this->get('/register')->assertRedirect('/login');
});

test('send code stores pending workos session in the Laravel session', function () {
    configureWorkosForTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->with('chris@openagents.com')
        ->andReturn((object) [
            'userId' => 'user_abc123',
        ]);

    $response = $this->post('/login/email', [
        'email' => 'chris@openagents.com',
    ]);

    $response->assertRedirect('/login');
    $response->assertSessionHas('status', 'code-sent');
    $response->assertSessionHas('auth.magic_auth.email', 'chris@openagents.com');
    $response->assertSessionHas('auth.magic_auth.user_id', 'user_abc123');
});

test('verify code signs in user and stores workos tokens', function () {
    configureWorkosForTests();

    $workosUser = (object) [
        'id' => 'user_abc123',
        'email' => 'chris@openagents.com',
        'firstName' => 'Chris',
        'lastName' => 'David',
        'profilePictureUrl' => 'https://example.com/avatar.png',
    ];

    $authResponse = (object) [
        'user' => $workosUser,
        'accessToken' => 'access_token_123',
        'refreshToken' => 'refresh_token_123',
    ];

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('authenticateWithMagicAuth')
        ->once()
        ->with('client_test_123', '123456', 'user_abc123', \Mockery::any(), \Mockery::any())
        ->andReturn($authResponse);

    $response = $this
        ->withSession([
            'auth.magic_auth' => [
                'email' => 'chris@openagents.com',
                'user_id' => 'user_abc123',
            ],
        ])
        ->post('/login/verify', [
            'code' => '123456',
        ]);

    $response->assertRedirect('/chat');
    $this->assertAuthenticated();

    $this->assertDatabaseHas('users', [
        'email' => 'chris@openagents.com',
        'workos_id' => 'user_abc123',
        'name' => 'Chris David',
    ]);

    $response->assertSessionHas('workos_access_token', 'access_token_123');
    $response->assertSessionHas('workos_refresh_token', 'refresh_token_123');
});

test('verify code requires pending email-code session', function () {
    configureWorkosForTests();

    $response = $this->post('/login/verify', [
        'code' => '123456',
    ]);

    $response->assertSessionHasErrors('code');
    $this->assertGuest();
});

test('logged-in users hitting login are redirected to home', function () {
    $this->actingAs(User::factory()->create());

    $this->get('/login')->assertRedirect('/');
});
