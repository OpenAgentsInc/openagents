<?php

use App\Models\User;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

it('requires sanctum authentication for api routes', function () {
    $this->getJson('/api/me')->assertUnauthorized();
});

it('supports me and personal access token lifecycle via api', function () {
    $user = User::factory()->create([
        'email' => 'token-user@openagents.com',
        'name' => 'Token User',
    ]);

    $seedToken = $user->createToken('seed-token')->plainTextToken;

    $this->withToken($seedToken)
        ->getJson('/api/me')
        ->assertOk()
        ->assertJsonPath('data.user.email', 'token-user@openagents.com')
        ->assertJsonPath('data.user.name', 'Token User');

    $expiresAt = Carbon::now()->addDays(7)->toIso8601String();

    $createResponse = $this->withToken($seedToken)
        ->postJson('/api/tokens', [
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
        ->getJson('/api/tokens')
        ->assertOk();

    $tokens = collect($listResponse->json('data'));
    expect($tokens)->not->toBeEmpty();

    $createdToken = $tokens->firstWhere('name', 'api-cli');
    expect($createdToken)->not->toBeNull();

    $this->withToken($seedToken)
        ->deleteJson('/api/tokens/'.$createdToken['id'])
        ->assertOk()
        ->assertJsonPath('data.deleted', true);

    $this->withToken($seedToken)
        ->deleteJson('/api/tokens')
        ->assertOk()
        ->assertJsonPath('data.deletedCount', 1);

    $this->withToken($seedToken)
        ->getJson('/api/tokens')
        ->assertOk()
        ->assertJsonCount(0, 'data');

    $currentTokenResult = $user->createToken('current-token');
    $currentToken = $currentTokenResult->plainTextToken;
    $currentTokenId = (int) $currentTokenResult->accessToken->id;

    $this->withToken($currentToken)
        ->deleteJson('/api/tokens/current')
        ->assertOk()
        ->assertJsonPath('data.deleted', true);

    expect($user->tokens()->where('id', $currentTokenId)->exists())->toBeFalse();
});

it('returns non-empty threads plus only the newest empty thread in /api/me', function () {
    $user = User::factory()->create([
        'email' => 'thread-filter-user@openagents.com',
    ]);
    $otherUser = User::factory()->create();

    $token = $user->createToken('thread-filter-token')->plainTextToken;

    $now = now();

    $emptyOldId = (string) Str::uuid7();
    $emptyNewestId = (string) Str::uuid7();
    $withMessagesId = (string) Str::uuid7();
    $otherUserThreadId = (string) Str::uuid7();

    DB::table('threads')->insert([
        [
            'id' => $emptyOldId,
            'user_id' => $user->id,
            'title' => 'Old empty',
            'created_at' => $now->copy()->subMinutes(30),
            'updated_at' => $now->copy()->subMinutes(30),
        ],
        [
            'id' => $withMessagesId,
            'user_id' => $user->id,
            'title' => 'Has messages',
            'created_at' => $now->copy()->subMinutes(10),
            'updated_at' => $now->copy()->subMinutes(2),
        ],
        [
            'id' => $emptyNewestId,
            'user_id' => $user->id,
            'title' => 'Newest empty',
            'created_at' => $now->copy()->subMinute(),
            'updated_at' => $now->copy()->subMinute(),
        ],
        [
            'id' => $otherUserThreadId,
            'user_id' => $otherUser->id,
            'title' => 'Other user thread',
            'created_at' => $now->copy()->subMinute(),
            'updated_at' => $now->copy()->subMinute(),
        ],
    ]);

    DB::table('messages')->insert([
        [
            'id' => (string) Str::uuid7(),
            'thread_id' => $withMessagesId,
            'run_id' => null,
            'user_id' => $user->id,
            'role' => 'user',
            'content' => 'hello',
            'meta' => null,
            'created_at' => $now->copy()->subMinutes(2),
            'updated_at' => $now->copy()->subMinutes(2),
        ],
        [
            'id' => (string) Str::uuid7(),
            'thread_id' => $otherUserThreadId,
            'run_id' => null,
            'user_id' => $otherUser->id,
            'role' => 'user',
            'content' => 'ignore',
            'meta' => null,
            'created_at' => $now->copy()->subMinute(),
            'updated_at' => $now->copy()->subMinute(),
        ],
    ]);

    $response = $this->withToken($token)
        ->getJson('/api/me')
        ->assertOk();

    $threadIds = collect($response->json('data.chatThreads'))
        ->pluck('id')
        ->all();

    expect($threadIds)->toBe([$emptyNewestId, $withMessagesId]);
    expect($threadIds)->not->toContain($emptyOldId);
    expect($threadIds)->not->toContain($otherUserThreadId);
});
