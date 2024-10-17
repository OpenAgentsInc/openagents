<?php

namespace Tests\Unit;

use App\Models\Project;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class ProjectTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function a_project_belongs_to_a_user()
    {
        $user = User::factory()->create();
        $project = Project::factory()->create(['user_id' => $user->id]);

        $this->assertInstanceOf(User::class, $project->user);
        $this->assertEquals($user->id, $project->user->id);
    }

    /** @test */
    public function a_project_has_many_threads()
    {
        $project = Project::factory()->create();
        $threads = Thread::factory()->count(3)->create(['project_id' => $project->id]);

        $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $project->threads);
        $this->assertCount(3, $project->threads);
    }
}