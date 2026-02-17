<?php

use App\Models\User;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

test('guests are routed to the in-chat onboarding flow', function () {
    $response = $this->get('/chat');

    $response->assertRedirect();
    $location = (string) $response->headers->get('Location');

    expect($location)->toContain('/chat/guest-');

    $this->get($location)->assertOk();
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
