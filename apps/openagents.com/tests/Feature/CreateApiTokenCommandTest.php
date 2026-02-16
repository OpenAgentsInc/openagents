<?php

use App\Models\User;
use Illuminate\Support\Facades\Artisan;

it('creates a sanctum token with multiple abilities via ops:create-api-token', function () {
    $user = User::factory()->create([
        'email' => 'command-token@openagents.com',
    ]);

    $exitCode = Artisan::call('ops:create-api-token', [
        'email' => 'command-token@openagents.com',
        'name' => 'command-test-token',
        '--abilities' => ['chat:read', 'chat:write'],
        '--expires-days' => '30',
    ]);

    expect($exitCode)->toBe(0);

    $output = Artisan::output();
    expect($output)->toContain('Token created successfully. Copy now; it will not be shown again:');
    expect($output)->toContain('name=command-test-token');

    $user->refresh();

    $token = $user->tokens()->latest('id')->first();

    expect($token)->not->toBeNull();
    expect($token->name)->toBe('command-test-token');
    expect($token->abilities)->toBe(['chat:read', 'chat:write']);
    expect($token->expires_at)->not->toBeNull();
});

it('returns a non-zero exit code when ops:create-api-token user is missing', function () {
    $exitCode = Artisan::call('ops:create-api-token', [
        'email' => 'missing@openagents.com',
        'name' => 'missing-user-token',
    ]);

    expect($exitCode)->toBe(1);

    $output = Artisan::output();
    expect($output)->toContain('User not found for email: missing@openagents.com');
});
