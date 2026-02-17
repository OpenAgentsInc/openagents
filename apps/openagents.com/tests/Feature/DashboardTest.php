<?php

use App\Models\User;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

test('guests can access in-chat onboarding without guest-id redirect', function () {
    $response = $this->get('/chat');

    $response->assertOk();
    $response->assertDontSee('/chat/guest-');
    $response->assertSee("enter your email and I'll send a one-time code");

    $guestConversationId = session('chat.guest.conversation_id');
    expect($guestConversationId)->toBeString()->and($guestConversationId)->toStartWith('guest-');
});

test('authenticated users can visit chat', function () {
    $this->actingAs(User::factory()->create());

    $this->get('/chat')->assertRedirect();
});

test('chat routes still work when threads table lacks autopilot_id', function () {
    $user = User::factory()->create();

    DB::statement('DROP INDEX IF EXISTS threads_autopilot_id_index');

    Schema::table('threads', function (Blueprint $table) {
        $table->dropColumn('autopilot_id');
    });

    $this->actingAs($user)
        ->get('/chat')
        ->assertRedirectContains('/chat/');
});
