<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use WorkOS\Exception\GenericException;

afterEach(function () {
    \Mockery::close();
});

beforeEach(function () {
    config()->set('lightning.spark_executor.base_url', '');
    config()->set('lightning.spark_executor.auth_token', '');
    config()->set('lightning.agent_wallets.auto_provision_on_auth', true);
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

test('send code JSON endpoint supports in-chat onboarding flow', function () {
    configureWorkosForTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->with('chris@openagents.com')
        ->andReturn((object) [
            'userId' => 'user_abc123',
        ]);

    $response = $this->postJson('/api/auth/email', [
        'email' => 'chris@openagents.com',
    ]);

    $response->assertOk()
        ->assertJsonPath('status', 'code-sent')
        ->assertJsonPath('email', 'chris@openagents.com');

    $response->assertSessionHas('auth.magic_auth.email', 'chris@openagents.com');
    $response->assertSessionHas('auth.magic_auth.user_id', 'user_abc123');
});

test('send code surfaces provider errors for chat onboarding', function () {
    configureWorkosForTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->andThrow(new GenericException('provider down'));

    $response = $this->postJson('/api/auth/email', [
        'email' => 'chris@openagents.com',
    ]);

    $response->assertStatus(422)
        ->assertJsonValidationErrors('email');
});

test('send code rejects invalid provider payload without user id', function () {
    configureWorkosForTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->andReturn((object) []);

    $response = $this->postJson('/api/auth/email', [
        'email' => 'chris@openagents.com',
    ]);

    $response->assertStatus(422)
        ->assertJsonValidationErrors('email');
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

    $response->assertRedirect('/');
    $this->assertAuthenticated();

    $this->assertDatabaseHas('users', [
        'email' => 'chris@openagents.com',
        'workos_id' => 'user_abc123',
        'name' => 'Chris David',
    ]);

    $response->assertSessionHas('workos_access_token', 'access_token_123');
    $response->assertSessionHas('workos_refresh_token', 'refresh_token_123');
});

test('verify code JSON endpoint signs in user for chat onboarding', function () {
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
        ->postJson('/api/auth/verify', [
            'code' => '123456',
        ]);

    $response->assertOk()
        ->assertJsonPath('status', 'authenticated')
        ->assertJsonPath('redirect', '/')
        ->assertJsonPath('user.email', 'chris@openagents.com');

    $this->assertAuthenticated();

    $response->assertSessionHas('workos_access_token', 'access_token_123');
    $response->assertSessionHas('workos_refresh_token', 'refresh_token_123');
});

test('verify code auto provisions a spark wallet when executor is configured', function () {
    configureWorkosForTests();

    config()->set('lightning.spark_executor.base_url', 'https://spark-executor.test');
    config()->set('lightning.spark_executor.auth_token', 'spark-token');

    Http::fake([
        'https://spark-executor.test/wallets/create' => Http::response([
            'ok' => true,
            'result' => [
                'mnemonic' => 'abandon ability able about above absent absorb abstract absurd abuse access accident',
                'sparkAddress' => 'chris@spark.wallet',
                'lightningAddress' => 'chris@lightning.openagents.com',
                'identityPubkey' => '02abc123',
                'balanceSats' => 0,
            ],
        ], 200),
    ]);

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
        ->postJson('/api/auth/verify', [
            'code' => '123456',
        ]);

    $response->assertOk()->assertJsonPath('status', 'authenticated');

    $user = User::query()->where('email', 'chris@openagents.com')->firstOrFail();

    expect(DB::table('user_spark_wallets')->where('user_id', $user->id)->exists())->toBeTrue();

    Http::assertSent(function (\Illuminate\Http\Client\Request $request): bool {
        return $request->url() === 'https://spark-executor.test/wallets/create'
            && $request->method() === 'POST';
    });
});

test('verify code surfaces provider errors for chat onboarding', function () {
    configureWorkosForTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('authenticateWithMagicAuth')
        ->once()
        ->andThrow(new GenericException('invalid code'));

    $response = $this
        ->withSession([
            'auth.magic_auth' => [
                'email' => 'chris@openagents.com',
                'user_id' => 'user_abc123',
            ],
        ])
        ->postJson('/api/auth/verify', [
            'code' => '123456',
        ]);

    $response->assertStatus(422)
        ->assertJsonValidationErrors('code');

    $this->assertGuest();
});

test('verify code requires pending email-code session', function () {
    configureWorkosForTests();

    $response = $this->post('/login/verify', [
        'code' => '123456',
    ]);

    $response->assertSessionHasErrors('code');
    $this->assertGuest();
});

test('verify code JSON requires pending email-code session', function () {
    configureWorkosForTests();

    $response = $this->postJson('/api/auth/verify', [
        'code' => '123456',
    ]);

    $response->assertStatus(422)
        ->assertJsonValidationErrors('code');

    $this->assertGuest();
});

test('logged-in users hitting login are redirected by guest middleware', function () {
    $this->actingAs(User::factory()->create());
    $this->get('/login')->assertRedirect('/');
});

test('verify code signs into existing email user when workos id is already linked elsewhere', function () {
    configureWorkosForTests();

    $emailOwner = User::factory()->create([
        'name' => 'Chris Email Owner',
        'email' => 'chris@openagents.com',
        'workos_id' => 'legacy_workos_user',
    ]);

    User::factory()->create([
        'name' => 'Imported Placeholder',
        'email' => 'placeholder+conflict@openagents.local',
        'workos_id' => 'user_abc123',
    ]);

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
        ->postJson('/api/auth/verify', [
            'code' => '123456',
        ]);

    $response->assertOk()
        ->assertJsonPath('status', 'authenticated')
        ->assertJsonPath('user.email', 'chris@openagents.com');

    $this->assertAuthenticatedAs($emailOwner);

    $this->assertDatabaseHas('users', [
        'id' => $emailOwner->id,
        'email' => 'chris@openagents.com',
        'workos_id' => 'legacy_workos_user',
    ]);
});
