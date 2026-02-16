<?php

use App\Models\User;
use Illuminate\Support\Carbon;

it('requires sanctum authentication for api v1 routes', function () {
    $this->getJson('/api/v1/me')->assertUnauthorized();
});

it('supports me and personal access token lifecycle via api', function () {
    $user = User::factory()->create([
        'email' => 'token-user@openagents.com',
        'name' => 'Token User',
    ]);

    $seedToken = $user->createToken('seed-token')->plainTextToken;

    $this->withToken($seedToken)
        ->getJson('/api/v1/me')
        ->assertOk()
        ->assertJsonPath('data.user.email', 'token-user@openagents.com')
        ->assertJsonPath('data.user.name', 'Token User');

    $expiresAt = Carbon::now()->addDays(7)->toIso8601String();

    $createResponse = $this->withToken($seedToken)
        ->postJson('/api/v1/tokens', [
            'name' => 'api-cli',
            'abilities' => ['chat:read', 'chat:write'],
            'expires_at' => $expiresAt,
        ]);

    $createResponse->assertCreated()
        ->assertJsonPath('data.name', 'api-cli')
        ->assertJsonPath('data.abilities.0', 'chat:read')
        ->assertJsonPath('data.abilities.1', 'chat:write');

    $plainToken = $createResponse->json('data.token');
    expect($plainToken)->toBeString()->not->toBeEmpty();

    $listResponse = $this->withToken($seedToken)
        ->getJson('/api/v1/tokens')
        ->assertOk();

    $tokens = collect($listResponse->json('data'));
    expect($tokens)->not->toBeEmpty();

    $createdToken = $tokens->firstWhere('name', 'api-cli');
    expect($createdToken)->not->toBeNull();

    $this->withToken($seedToken)
        ->deleteJson('/api/v1/tokens/'.$createdToken['id'])
        ->assertOk()
        ->assertJsonPath('data.deleted', true);

    $this->withToken($seedToken)
        ->deleteJson('/api/v1/tokens')
        ->assertOk()
        ->assertJsonPath('data.deletedCount', 1);

    $this->withToken($seedToken)
        ->getJson('/api/v1/tokens')
        ->assertOk()
        ->assertJsonCount(0, 'data');
});
