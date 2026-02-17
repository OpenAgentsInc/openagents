<?php

use App\Models\User;
use Illuminate\Support\Facades\Config;

beforeEach(function () {
    config()->set('posthog.disabled', true);
    Config::set('admin.emails', ['chris@openagents.com']);
});

it('forbids non-admin users from api admin status endpoint', function () {
    $nonAdmin = User::factory()->create([
        'email' => 'user@openagents.com',
    ]);

    $nonAdminToken = $nonAdmin->createToken('non-admin')->plainTextToken;

    $this->withToken($nonAdminToken)
        ->getJson('/api/admin/status')
        ->assertForbidden();
});

it('allows admin users on api admin status endpoint', function () {
    $admin = User::factory()->create([
        'email' => 'chris@openagents.com',
    ]);

    $adminToken = $admin->createToken('admin')->plainTextToken;

    $this->withToken($adminToken)
        ->getJson('/api/admin/status')
        ->assertOk()
        ->assertJsonPath('data.status', 'ok')
        ->assertJsonPath('data.adminEmails.0', 'chris@openagents.com');
});

it('supports profile read, update, and delete via api', function () {
    $user = User::factory()->create([
        'email' => 'profile-user@openagents.com',
        'name' => 'Original Name',
    ]);

    $token = $user->createToken('profile')->plainTextToken;

    $this->withToken($token)
        ->getJson('/api/settings/profile')
        ->assertOk()
        ->assertJsonPath('data.email', 'profile-user@openagents.com')
        ->assertJsonPath('data.name', 'Original Name');

    $this->withToken($token)
        ->patchJson('/api/settings/profile', [
            'name' => 'Updated Name',
        ])
        ->assertOk()
        ->assertJsonPath('data.name', 'Updated Name');

    $this->withToken($token)
        ->deleteJson('/api/settings/profile', [
            'email' => 'wrong@openagents.com',
        ])
        ->assertUnprocessable()
        ->assertJsonPath('message', 'Email confirmation does not match the authenticated user.');

    $this->withToken($token)
        ->deleteJson('/api/settings/profile', [
            'email' => 'profile-user@openagents.com',
        ])
        ->assertOk()
        ->assertJsonPath('data.deleted', true);

    expect(User::query()->where('id', $user->id)->exists())->toBeFalse();
});
