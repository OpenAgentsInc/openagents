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
        // Create a user and some chat threads with messages
        $user = User::factory()->create();
        $thread1 = Thread::factory()->create(['user_id' => $user->id]);
        $thread2 = Thread::factory()->create(['user_id' => $user->id]);
        $thread3 = Thread::factory()->create(['user_id' => $user->id]);

        Message::factory()->count(3)->create(['thread_id' => $thread1->id]);
        Message::factory()->count(2)->create(['thread_id' => $thread2->id]);
        Message::factory()->count(1)->create(['thread_id' => $thread3->id]);

        // Simulate an HTMX request to load messages for a specific chat
        $response = $this->actingAs($user)
            ->withHeaders(['HX-Request' => 'true'])
            ->get("/chat/{$thread1->id}/messages");

        $response->assertStatus(200);
        $response->assertViewIs('partials.chat_messages');
        $response->assertViewHas('messages');
        $response->assertSee($thread1->messages[0]->content);
        $response->assertSee($thread1->messages[1]->content);
        $response->assertSee($thread1->messages[2]->content);
    }
}