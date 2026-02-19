<?php

use App\Models\User;
use App\Services\PostHogService;

it('captures a posthog pageview for successful html get requests', function () {
    $user = User::factory()->create([
        'email' => 'pageview-user@openagents.com',
    ]);

    $mock = Mockery::mock(PostHogService::class);
    $mock->shouldReceive('capture')
        ->once()
        ->withArgs(function (string $distinctId, string $event, array $properties): bool {
            return $distinctId === 'pageview-user@openagents.com'
                && $event === '$pageview'
                && ($properties['$pathname'] ?? null) === '/'
                && ($properties['auth_state'] ?? null) === 'authenticated'
                && ($properties['source'] ?? null) === 'laravel_web_middleware';
        });

    app()->instance(PostHogService::class, $mock);

    $this->actingAs($user)
        ->get('/')
        ->assertOk();
});

it('does not capture posthog pageview for unauthenticated requests', function () {
    $mock = Mockery::mock(PostHogService::class);
    $mock->shouldNotReceive('capture');

    app()->instance(PostHogService::class, $mock);

    $this->get('/')->assertOk();
});

it('does not capture posthog pageview for api paths', function () {
    $mock = Mockery::mock(PostHogService::class);
    $mock->shouldNotReceive('capture');

    app()->instance(PostHogService::class, $mock);

    $this->getJson('/api/chat/guest-session')
        ->assertOk()
        ->assertJsonStructure(['conversationId']);
});
