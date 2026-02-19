<?php

use App\AI\Runtime\RuntimeDriverRouter;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

test('runtime router honors force legacy rollback switch above all routing controls', function () {
    config()->set('runtime.driver', 'elixir');
    config()->set('runtime.rollback.force_legacy', true);
    config()->set('runtime.overrides.enabled', true);

    $user = User::factory()->create();
    $threadId = createRuntimeTestThread($user->id);

    DB::table('runtime_driver_overrides')->insert([
        'id' => (string) Str::uuid7(),
        'scope_type' => 'user',
        'scope_id' => (string) $user->id,
        'driver' => 'elixir',
        'is_active' => true,
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $router = resolve(RuntimeDriverRouter::class);
    $driver = $router->resolveDriver($user->id, $threadId);

    expect($driver)->toBe('legacy');
});

test('runtime router applies db user override driver', function () {
    config()->set('runtime.driver', 'legacy');
    config()->set('runtime.rollback.force_legacy', false);
    config()->set('runtime.overrides.enabled', true);

    $user = User::factory()->create();
    $threadId = createRuntimeTestThread($user->id);

    DB::table('runtime_driver_overrides')->insert([
        'id' => (string) Str::uuid7(),
        'scope_type' => 'user',
        'scope_id' => (string) $user->id,
        'driver' => 'elixir',
        'is_active' => true,
        'reason' => 'user canary',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $router = resolve(RuntimeDriverRouter::class);
    $driver = $router->resolveDriver($user->id, $threadId);

    expect($driver)->toBe('elixir');
});

test('runtime router applies db autopilot override driver', function () {
    config()->set('runtime.driver', 'legacy');
    config()->set('runtime.rollback.force_legacy', false);
    config()->set('runtime.overrides.enabled', true);

    $user = User::factory()->create();
    $autopilotId = (string) Str::uuid7();
    $threadId = createRuntimeTestThread($user->id, $autopilotId);

    DB::table('runtime_driver_overrides')->insert([
        'id' => (string) Str::uuid7(),
        'scope_type' => 'autopilot',
        'scope_id' => $autopilotId,
        'driver' => 'elixir',
        'is_active' => true,
        'reason' => 'autopilot canary',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $router = resolve(RuntimeDriverRouter::class);
    $driver = $router->resolveDriver($user->id, $threadId);

    expect($driver)->toBe('elixir');
});

test('runtime router supports deterministic canary percentage routing', function () {
    config()->set('runtime.driver', 'legacy');
    config()->set('runtime.rollback.force_legacy', false);
    config()->set('runtime.overrides.enabled', false);
    config()->set('runtime.canary.seed', 'test-seed');
    config()->set('runtime.canary.user_percent', 100);

    $user = User::factory()->create();
    $threadId = createRuntimeTestThread($user->id);

    $router = resolve(RuntimeDriverRouter::class);
    expect($router->resolveDriver($user->id, $threadId))->toBe('elixir');

    config()->set('runtime.canary.user_percent', 0);
    expect($router->resolveDriver($user->id, $threadId))->toBe('legacy');
});

function createRuntimeTestThread(int $userId, ?string $autopilotId = null): string
{
    $threadId = (string) Str::uuid7();

    DB::table('threads')->insert([
        'id' => $threadId,
        'user_id' => $userId,
        'autopilot_id' => $autopilotId,
        'title' => 'Routing test thread',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    return $threadId;
}
