<?php

namespace Tests\Unit;

use App\Models\Thread;
use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Message;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class ThreadTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function a_thread_belongs_to_a_user()
    {
        $user = User::factory()->create();
        $thread = Thread::factory()->create(['user_id' => $user->id]);

        $this->assertInstanceOf(User::class, $thread->user);
        $this->assertEquals($user->id, $thread->user->id);
    }

    /** @test */
    public function a_thread_belongs_to_a_team()
    {
        $team = Team::factory()->create();
        $thread = Thread::factory()->create(['team_id' => $team->id]);

        $this->assertInstanceOf(Team::class, $thread->team);
        $this->assertEquals($team->id, $thread->team->id);
    }

    /** @test */
    public function a_thread_belongs_to_a_project()
    {
        $project = Project::factory()->create();
        $thread = Thread::factory()->create(['project_id' => $project->id]);

        $this->assertInstanceOf(Project::class, $thread->project);
        $this->assertEquals($project->id, $thread->project->id);
    }

    /** @test */
    public function a_thread_has_many_messages()
    {
        $thread = Thread::factory()->create();
        $messages = Message::factory()->count(3)->create(['thread_id' => $thread->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $thread->messages);
        $this->assertCount(3, $thread->messages);
    }
}