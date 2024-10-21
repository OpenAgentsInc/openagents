<?php

namespace Database\Seeders;

use Illuminate\Database\Seeder;
use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;

class DatabaseSeeder extends Seeder
{
    public function run(): void
    {
        // Create a demo user
        $user = User::factory()->create([
            'name' => 'Demo User',
            'email' => 'demo@example.com',
        ]);

        // Create a team for the user
        $team = Team::factory()->create([
            'name' => 'Demo Team',
            'user_id' => $user->id,
        ]);

        // Attach the user to the team
        $user->teams()->attach($team);

        // Set the user's current team
        $user->current_team_id = $team->id;
        $user->save();

        // Create projects for the team
        $projects = Project::factory(3)->create([
            'team_id' => $team->id,
        ]);

        // Create threads and messages for each project
        foreach ($projects as $project) {
            $threads = Thread::factory(3)->create([
                'project_id' => $project->id,
                'user_id' => $user->id,
            ]);

            foreach ($threads as $thread) {
                Message::factory(5)->create([
                    'thread_id' => $thread->id,
                    'user_id' => $user->id,
                ]);
            }
        }

        // Create some personal threads for the user
        $personalThreads = Thread::factory(2)->create([
            'user_id' => $user->id,
            'project_id' => null,
        ]);

        foreach ($personalThreads as $thread) {
            Message::factory(3)->create([
                'thread_id' => $thread->id,
                'user_id' => $user->id,
            ]);
        }
    }
}