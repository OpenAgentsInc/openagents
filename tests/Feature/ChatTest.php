<?php

// 2. user can create a conversation
test('user can create a conversation', function() {
  // Given we have an authenticated user
  $user = User::factory()->create();
  $this->actingAs($user);

  // And a user to message
  $recipient = User::factory()->create();

  // When we visit the conversations page
  $response = $this->get('/conversations');

  // Then we should see the recipient's name
  $response->assertSee($recipient->name);

  // When we click the recipient's name
  $response = $this->get('/conversations/' . $recipient->id);

  // Then we should see the messages page
  $response->assertSee('Messages');

  // And when we type a message and click send
  $response = $this->post('/conversations/' . $recipient->id, [
    'message' => 'My first message'
  ]);

  // Then we should see the message
  $response->assertSee('My first message');
});

// 3. user can send a message
// 4. agent can send a message
