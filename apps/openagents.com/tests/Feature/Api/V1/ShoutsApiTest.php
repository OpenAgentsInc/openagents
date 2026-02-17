<?php

use App\Models\Shout;
use App\Models\User;
use Illuminate\Support\Facades\DB;

it('creates shouts and lists feed with zone filtering', function () {
    $author = User::factory()->create([
        'email' => 'shout-author@openagents.com',
    ]);

    $other = User::factory()->create([
        'email' => 'shout-other@openagents.com',
    ]);

    $token = $author->createToken('shout-writer', ['shouts:write'])->plainTextToken;

    $this->withToken($token)
        ->postJson('/api/shouts', [
            'body' => 'L402 payment shipped',
            'zone' => 'L402',
        ])
        ->assertCreated()
        ->assertJsonPath('data.zone', 'l402')
        ->assertJsonPath('data.body', 'L402 payment shipped')
        ->assertJsonPath('data.author.handle', $author->handle);

    Shout::query()->create([
        'user_id' => $other->id,
        'zone' => 'global',
        'body' => 'hello from global',
        'visibility' => 'public',
    ]);

    $this->getJson('/api/shouts?zone=l402')
        ->assertOk()
        ->assertJsonCount(1, 'data')
        ->assertJsonPath('data.0.zone', 'l402');

    $this->getJson('/api/shouts/zones')
        ->assertOk()
        ->assertJsonPath('data.0.zone', 'global');
});

it('caps shout limit and supports before_id pagination', function () {
    $author = User::factory()->create([
        'email' => 'shout-pagination@openagents.com',
    ]);

    $now = now();

    $rows = [];
    for ($i = 0; $i < 205; $i++) {
        $rows[] = [
            'user_id' => $author->id,
            'zone' => 'global',
            'body' => 'feed-'.$i,
            'visibility' => 'public',
            'created_at' => $now->copy()->subSeconds($i),
            'updated_at' => $now->copy()->subSeconds($i),
        ];
    }

    DB::table('shouts')->insert($rows);

    $pageOne = $this->getJson('/api/shouts?limit=999')
        ->assertOk();

    $pageOne->assertJsonCount(200, 'data');

    $lastId = $pageOne->json('data.199.id');
    expect($lastId)->toBeInt();

    $pageTwo = $this->getJson('/api/shouts?limit=200&before_id='.$lastId)
        ->assertOk();

    $pageTwo->assertJsonCount(5, 'data');
});
