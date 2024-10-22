<?php

namespace Tests\Feature;

use App\Models\User;
use App\Models\Thread;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class HomepageChatTest extends TestCase
{
    use RefreshDatabase;

    public function test_authenticated_user_can_send_message_from_homepage_and_is_redirected_to_new_chat_thread()
    {
        $user = User::factory()->create();

        $response = $this->actingAs($user)
            ->post('/send-message', [
                'message' => 'Test message from homepage'
            ]);

        $response->assertStatus(302); // Assert that there's a redirect

        $thread = Thread::latest()->first();
        $response->assertRedirect("/chat/{$thread->id}");

        $this->assertDatabaseHas('messages', [
            'user_id' => $user->id,
            'content' => 'Test message from homepage'
        ]);

        $this->assertDatabaseHas('threads', [
            'user_id' => $user->id,
            'title' => 'Test message from homepage...'
        ]);
    }

    public function test_unauthenticated_user_is_redirected_to_login_when_trying_to_send_message_from_homepage()
    {
        $response = $this->post('/send-message', [
            'message' => 'Test message from homepage'
        ]);

        $response->assertStatus(302);
        $response->assertRedirect('/login');
    }

    public function test_chat_page_loads_correctly_after_sending_message()
    {
        $user = User::factory()->create();

        $response = $this->actingAs($user)
            ->post('/send-message', [
                'message' => 'Another test message'
            ]);

        $thread = Thread::latest()->first();

        $chatResponse = $this->actingAs($user)->get("/chat/{$thread->id}");
        $chatResponse->assertStatus(200);
        $chatResponse->assertSee('Another test message');
        $chatResponse->assertSee($thread->title);
        $chatResponse->assertSee('Send'); // Assuming there's a "Send" button on the chat page
    }
}
