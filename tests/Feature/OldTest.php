<?php

test('chat route returns redirect to agent/1/chat', function () {
    $this->seed(ConciergeSeeder::class);

    $this->get(route('chat'))
         ->assertRedirect(route('agent.chat', ['id' => 1]));
});
