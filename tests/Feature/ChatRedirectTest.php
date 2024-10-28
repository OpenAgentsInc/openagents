<?php

use App\Models\User;

// /chat needs to redirect to /chat/1 (assuming user owns it)
test('visiting /chat redirects to appropriate thread', function () {

    // Assert there are no threads in the database
    $this->assertDatabaseCount('threads', 0);

    $user = User::factory()->create();
    $response = $this->actingAs($user)->get('/chat');
    $response->assertRedirect('/chat/1');

    // Assert there is now one thread in the database
    $this->assertDatabaseCount('threads', 1);
});
