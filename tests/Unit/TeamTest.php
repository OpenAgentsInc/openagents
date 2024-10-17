<?php

namespace Tests\Unit;

use App\Models\Team;
use App\Models\User;
use App\Models\Thread;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class TeamTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function a_team_has_many_users()
    {
        $team = Team::factory()->create();
        $users = User::factory()->count(3)->create(['team_id' => $team->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $team->users);
        $this->assertCount(3, $team->users);
    }

    /** @test */
    public function a_team_has_many_threads()
    {
        $team = Team::factory()->create();
        $threads = Thread::factory()->count(3)->create(['team_id' => $team->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $team->threads);
        $this->assertCount(3, $team->threads);
    }
}