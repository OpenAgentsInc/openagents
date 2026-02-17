<?php

use Illuminate\Testing\TestResponse;

/**
 * @return array<string, mixed>
 */
function inertiaPayload(TestResponse $response): array
{
    $html = (string) $response->getContent();

    preg_match('/data-page="([^"]+)"/', $html, $matches);

    expect($matches)->toHaveKey(1);

    $json = html_entity_decode((string) $matches[1], ENT_QUOTES);

    /** @var array<string, mixed> $payload */
    $payload = json_decode($json, true, 512, JSON_THROW_ON_ERROR);

    return $payload;
}

test('guest chat keeps a stable guest conversation id in session', function () {
    $first = $this->get('/chat');
    $first->assertOk();

    $firstPayload = inertiaPayload($first);
    $firstConversationId = (string) ($firstPayload['props']['conversationId'] ?? '');

    expect($firstConversationId)->toMatch('/^g-[a-f0-9]{32}$/');
    expect(session('chat.guest.conversation_id'))->toBe($firstConversationId);

    $second = $this->get('/chat');
    $second->assertOk();

    $secondPayload = inertiaPayload($second);
    $secondConversationId = (string) ($secondPayload['props']['conversationId'] ?? '');

    expect($secondConversationId)->toBe($firstConversationId);

    // A mismatched guest conversation id in URL should not redirect-loop.
    $mismatch = $this->get('/chat/guest-does-not-match');
    $mismatch->assertOk();

    $mismatchPayload = inertiaPayload($mismatch);
    expect((string) ($mismatchPayload['props']['conversationId'] ?? ''))
        ->toBe($firstConversationId);
});

test('guest chat shows verification step when pending email exists', function () {
    $chat = $this->withSession([
        'auth.magic_auth' => [
            'email' => 'chris@openagents.com',
            'user_id' => 'user_abc123',
        ],
    ])->get('/chat');

    $chat->assertOk();

    $payload = inertiaPayload($chat);
    $props = $payload['props'] ?? [];

    expect($props['guestOnboarding']['enabled'] ?? null)->toBeTrue();
    expect($props['guestOnboarding']['step'] ?? null)->toBe('code');
    expect($props['guestOnboarding']['pendingEmail'] ?? null)->toBe('chris@openagents.com');
    expect((string) ($props['initialMessages'][0]['content'] ?? ''))
        ->toContain('Enter your 6-digit verification code');
});

test('guest chat falls back to email step when pending auth session is malformed', function () {
    $chat = $this->withSession([
        'auth.magic_auth' => 'invalid-structure',
    ])->get('/chat');

    $chat->assertOk();

    $payload = inertiaPayload($chat);
    $props = $payload['props'] ?? [];

    expect($props['guestOnboarding']['enabled'] ?? null)->toBeTrue();
    expect($props['guestOnboarding']['step'] ?? null)->toBe('email');
    expect($props['guestOnboarding']['pendingEmail'] ?? null)->toBeNull();
    expect((string) ($props['initialMessages'][0]['content'] ?? ''))
        ->toContain("enter your email and I'll send a one-time code");
});
