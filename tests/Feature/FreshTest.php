<?php

namespace Tests\Feature;

use App\Models\User;
use App\Models\Thread;
use App\Models\Message;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class FreshTest extends TestCase
{
    use RefreshDatabase;

    public function test_fresh_page_loads_correctly()
    {
        $user = User::factory()->create();
        $response = $this->actingAs($user)->get('/fresh');
        $response->assertStatus(200);
        $response->assertViewIs('fresh');
    }

    public function test_clicking_chat_loads_messages_in_main_area()
    {
        $user = User::factory()->create();
        $thread = Thread::factory()->create(['user_id' => $user->id]);
        Message::factory()->count(3)->create(['thread_id' => $thread->id]);

        $response = $this->actingAs($user)
            ->withHeaders(['HX-Request' => 'true'])
            ->get("/chat/{$thread->id}/messages");

        $response->assertStatus(200);
        $response->assertViewIs('partials.chat_messages');
        $response->assertViewHas('messages');
        $response->assertSee($thread->messages[0]->content);
        $response->assertSee($thread->messages[1]->content);
        $response->assertSee($thread->messages[2]->content);
    }

    public function test_fresh_page_shows_user_threads()
    {
        $user = User::factory()->create();
        $threads = Thread::factory()->count(3)->create(['user_id' => $user->id]);

        $response = $this->actingAs($user)->get('/fresh');

        $response->assertStatus(200);
        $response->assertViewIs('fresh');
        foreach ($threads as $thread) {
            $response->assertSee($thread->title);
        }
    }

    public function test_sending_message_adds_to_thread()
    {
        $user = User::factory()->create();
        $thread = Thread::factory()->create(['user_id' => $user->id]);

        $response = $this->actingAs($user)
            ->withHeaders(['HX-Request' => 'true'])
            ->post("/chat/{$thread->id}/send", [
                'content' => 'Test message content'
            ]);

        $response->assertStatus(200);
        $response->assertViewIs('partials.chat_messages');
        $response->assertSee('Test message content');

        $this->assertDatabaseHas('messages', [
            'thread_id' => $thread->id,
            'user_id' => $user->id,
            'content' => 'Test message content'
        ]);
    }

    public function test_unauthorized_user_cannot_access_fresh_page()
    {
        $response = $this->get('/fresh');
        $response->assertStatus(302);
        $response->assertRedirect('/login');
    }

    public function test_unauthorized_user_cannot_send_message()
    {
        $thread = Thread::factory()->create();

        $response = $this->post("/chat/{$thread->id}/send", [
            'content' => 'Test message content'
        ]);

        $response->assertStatus(302);
        $response->assertRedirect('/login');
    }

    public function test_user_cannot_access_other_users_threads()
    {
        $user1 = User::factory()->create();
        $user2 = User::factory()->create();
        $thread = Thread::factory()->create(['user_id' => $user2->id]);

        $response = $this->actingAs($user1)
            ->withHeaders(['HX-Request' => 'true'])
            ->get("/chat/{$thread->id}/messages");

        $response->assertStatus(403);
    }

    public function test_empty_message_is_not_sent()
    {
        $user = User::factory()->create();
        $thread = Thread::factory()->create(['user_id' => $user->id]);

        $response = $this->actingAs($user)
            ->withHeaders(['HX-Request' => 'true'])
            ->post("/chat/{$thread->id}/send", [
                'content' => ''
            ]);

        $response->assertStatus(422);
    }
}