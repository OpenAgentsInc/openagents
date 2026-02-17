<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;

it('returns a bounded guest conversation id and provisions guest rows', function () {
    $response = $this->getJson('/api/chat/guest-session');

    $response->assertOk();

    $conversationId = $response->json('conversationId');

    expect($conversationId)->toBeString();
    expect($conversationId)->toMatch('/^g-[a-f0-9]{32}$/');

    /** @var User $guest */
    $guest = User::query()->where('email', 'guest@openagents.internal')->firstOrFail();

    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();

    expect(DB::table('threads')
        ->where('id', $conversationId)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();
});

it('accepts a valid requested guest conversation id', function () {
    $requested = 'g-'.str_repeat('a', 32);

    $response = $this->getJson('/api/chat/guest-session?conversationId='.$requested);

    $response->assertOk()
        ->assertJsonPath('conversationId', $requested);

    expect(session('chat.guest.conversation_id'))->toBe($requested);
});

it('replaces legacy oversized guest ids in session with bounded ids', function () {
    $legacy = 'guest-019c6c64-bc20-7117-9a31-f1efd129c7a1';

    $response = $this
        ->withSession(['chat.guest.conversation_id' => $legacy])
        ->getJson('/api/chat/guest-session');

    $response->assertOk();

    $conversationId = $response->json('conversationId');

    expect($conversationId)->toBeString();
    expect($conversationId)->toMatch('/^g-[a-f0-9]{32}$/');
    expect($conversationId)->not->toBe($legacy);
    expect(session('chat.guest.conversation_id'))->toBe($conversationId);
});
