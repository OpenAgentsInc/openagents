<?php

namespace Tests\Unit;

use App\Models\Message;
use App\Models\User;
use App\Models\Thread;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class MessageTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function a_message_belongs_to_a_user()
    {
        $user = User::factory()->create();
        $message = Message::factory()->create(['user_id' => $user->id]);

        $this->assertInstanceOf(User::class, $message->user);
        $this->assertEquals($user->id, $message->user->id);
    }

    /** @test */
    public function a_message_belongs_to_a_thread()
    {
        $thread = Thread::factory()->create();
        $message = Message::factory()->create(['thread_id' => $thread->id]);

        $this->assertInstanceOf(Thread::class, $message->thread);
        $this->assertEquals($thread->id, $message->thread->id);
    }
}