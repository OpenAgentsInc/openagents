<?php

namespace Tests\Unit;

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class UserTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function a_user_belongs_to_a_team()
    {
        $team = Team::factory()->create();
        $user = User::factory()->create(['team_id' => $team->id]);

        $this->assertInstanceOf(Team::class, $user->team);
        $this->assertEquals($team->id, $user->team->id);
    }

    /** @test */
    public function a_user_has_many_projects()
    {
        $user = User::factory()->create();
        $projects = Project::factory()->count(3)->create(['user_id' => $user->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $user->projects);
        $this->assertCount(3, $user->projects);
    }

    /** @test */
    public function a_user_has_many_threads()
    {
        $user = User::factory()->create();
        $threads = Thread::factory()->count(3)->create(['user_id' => $user->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $user->threads);
        $this->assertCount(3, $user->threads);
    }

    /** @test */
    public function a_user_has_many_messages()
    {
        $user = User::factory()->create();
        $messages = Message::factory()->count(3)->create(['user_id' => $user->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $user->messages);
        $this->assertCount(3, $user->messages);
    }
}