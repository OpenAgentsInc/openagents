<?php

use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;

test('visiting private thread by non-member redirects to homepage', function () {
    $this->withoutExceptionHandling();

    $thread = Thread::factory()->private()->create();
    $agent = Agent::factory()->create();
    $thread->agents()->attach($agent);

    $response = $this->get("/chat/{$thread->id}");

    $response->assertRedirect('/');
});

test('visiting private thread by member shows chat', function () {
    $this->withoutExceptionHandling();

    $user = User::factory()->create();
    $this->actingAs($user);

    $thread = Thread::factory()->private()->create();
    $agent = Agent::factory()->create();
    $thread->agents()->attach($agent);
    $thread->users()->attach($user);

    $response = $this->get("/chat/{$thread->id}");

    $response->assertSuccessful();
});
