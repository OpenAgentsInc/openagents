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
    $first->assertRedirect();

    $guestLocation = (string) $first->headers->get('Location');
    expect($guestLocation)->toContain('/chat/guest-');

    $this->get($guestLocation)->assertOk();

    // A mismatched guest conversation id should be corrected to the session id.
    $mismatch = $this->get('/chat/guest-does-not-match');
    $mismatch->assertRedirect($guestLocation);
});

test('guest chat shows verification step when pending email exists', function () {
    $response = $this->withSession([
        'auth.magic_auth' => [
            'email' => 'chris@openagents.com',
            'user_id' => 'user_abc123',
        ],
    ])->get('/chat');

    $response->assertRedirect();
    $location = (string) $response->headers->get('Location');

    $chat = $this->get($location);
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
    $response = $this->withSession([
        'auth.magic_auth' => 'invalid-structure',
    ])->get('/chat');

    $response->assertRedirect();
    $location = (string) $response->headers->get('Location');

    $chat = $this->get($location);
    $chat->assertOk();

    $payload = inertiaPayload($chat);
    $props = $payload['props'] ?? [];

    expect($props['guestOnboarding']['enabled'] ?? null)->toBeTrue();
    expect($props['guestOnboarding']['step'] ?? null)->toBe('email');
    expect($props['guestOnboarding']['pendingEmail'] ?? null)->toBeNull();
    expect((string) ($props['initialMessages'][0]['content'] ?? ''))
        ->toContain("enter your email and I'll send a one-time code");
});
