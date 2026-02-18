<?php

use App\Models\User;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;
use Illuminate\Testing\TestResponse;

/**
 * @return array<string, mixed>
 */
function dashboardInertiaPayload(TestResponse $response): array
{
    $html = (string) $response->getContent();

    preg_match('/data-page="([^"]+)"/', $html, $matches);

    expect($matches)->toHaveKey(1);

    $json = html_entity_decode((string) $matches[1], ENT_QUOTES);

    /** @var array<string, mixed> $payload */
    $payload = json_decode($json, true, 512, JSON_THROW_ON_ERROR);

    return $payload;
}

test('guests can access chat route without guest-id redirect', function () {
    $response = $this->get('/chat');

    $response->assertOk();
    $response->assertDontSee('/chat/guest-');

    $payload = dashboardInertiaPayload($response);
    expect((string) ($payload['component'] ?? ''))->toBe('index');

    $guestConversationId = session('chat.guest.conversation_id');
    expect($guestConversationId)->toBeString()->and($guestConversationId)->toMatch('/^g-[a-f0-9]{32}$/');
});

test('authenticated users visiting chat root are redirected to homepage', function () {
    $this->actingAs(User::factory()->create());

    $this->get('/chat')->assertRedirect('/');
});

test('chat root route still works when threads table lacks autopilot_id', function () {
    $user = User::factory()->create();

    DB::statement('DROP INDEX IF EXISTS threads_autopilot_id_index');

    Schema::table('threads', function (Blueprint $table) {
        $table->dropColumn('autopilot_id');
    });

    $this->actingAs($user)
        ->get('/chat')
        ->assertRedirect('/');
});

test('home rehydrates authenticated user from chat auth session key', function () {
    $user = User::factory()->create([
        'email' => 'rehydrate-user@openagents.com',
    ]);

    $response = $this->withSession([
        'chat.auth_user_id' => (int) $user->id,
    ])->get('/');

    $response->assertOk();

    $payload = dashboardInertiaPayload($response);

    expect($payload['props']['auth']['user']['email'] ?? null)->toBe($user->email);
    expect(auth()->check())->toBeTrue();
});
