<?php

use App\Models\User;
use App\Models\Whisper;
use Illuminate\Support\Facades\DB;
use Laravel\Sanctum\Sanctum;

it('sends whisper by handle and restricts read access to participants', function () {
    $sender = User::factory()->create([
        'email' => 'whisper-sender@openagents.com',
        'handle' => 'sender-user',
    ]);

    $recipient = User::factory()->create([
        'email' => 'whisper-recipient@openagents.com',
        'handle' => 'recipient-user',
    ]);

    $outsider = User::factory()->create([
        'email' => 'whisper-outsider@openagents.com',
        'handle' => 'outsider-user',
    ]);

    Sanctum::actingAs($sender, ['whispers:write', 'whispers:read']);

    $create = $this->postJson('/api/whispers', [
        'recipientHandle' => 'recipient-user',
        'body' => 'hey from sender',
    ])
        ->assertCreated()
        ->assertJsonPath('data.sender.handle', 'sender-user')
        ->assertJsonPath('data.recipient.handle', 'recipient-user')
        ->assertJsonPath('data.body', 'hey from sender');

    $whisperId = (int) $create->json('data.id');

    Sanctum::actingAs($recipient, ['whispers:write', 'whispers:read']);

    $this->getJson('/api/whispers?with=sender-user')
        ->assertOk()
        ->assertJsonCount(1, 'data')
        ->assertJsonPath('data.0.id', $whisperId);

    Sanctum::actingAs($outsider, ['whispers:write', 'whispers:read']);

    $this->getJson('/api/whispers?with=sender-user')
        ->assertOk()
        ->assertJsonCount(0, 'data');

    $this->patchJson('/api/whispers/'.$whisperId.'/read')
        ->assertForbidden();

    Sanctum::actingAs($recipient, ['whispers:write', 'whispers:read']);

    $this->patchJson('/api/whispers/'.$whisperId.'/read')
        ->assertOk()
        ->assertJsonPath('data.id', $whisperId);

    expect(Whisper::query()->findOrFail($whisperId)->read_at)->not->toBeNull();
});

it('caps whisper limit and supports thread pagination', function () {
    $sender = User::factory()->create([
        'email' => 'whisper-limit-sender@openagents.com',
        'handle' => 'whisper-limit-sender',
    ]);

    $recipient = User::factory()->create([
        'email' => 'whisper-limit-recipient@openagents.com',
        'handle' => 'whisper-limit-recipient',
    ]);

    $now = now();
    $rows = [];

    for ($i = 0; $i < 205; $i++) {
        $rows[] = [
            'sender_id' => $sender->id,
            'recipient_id' => $recipient->id,
            'body' => 'message-'.$i,
            'read_at' => null,
            'created_at' => $now->copy()->subSeconds($i),
            'updated_at' => $now->copy()->subSeconds($i),
        ];
    }

    DB::table('whispers')->insert($rows);

    Sanctum::actingAs($sender, ['whispers:read']);

    $pageOne = $this->getJson('/api/whispers?with='.$recipient->handle.'&limit=999')
        ->assertOk();

    $pageOne->assertJsonCount(200, 'data');

    $lastId = $pageOne->json('data.199.id');
    expect($lastId)->toBeInt();

    $this->getJson('/api/whispers?with='.$recipient->handle.'&before_id='.$lastId.'&limit=200')
        ->assertOk()
        ->assertJsonCount(5, 'data');
});
