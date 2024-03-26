<?php

use App\Models\Agent;
use App\Models\Thread;

test('visiting private thread by non-member redirects to homepage', function () {
    $this->withoutExceptionHandling();

    $thread = Thread::factory()->private()->create();
    $agent = Agent::factory()->create();
    $thread->agents()->attach($agent);

    $response = $this->get("/chat/{$thread->id}");

    $response->assertRedirect('/');
});
