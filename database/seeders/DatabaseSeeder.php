<?php

namespace Database\Seeders;

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    /**
     * Seed the application's database.
     */
    public function run(): void
    {
        // Get the existing user with id 1
        $user = User::findOrFail(1);

        // Create demo teams
        $teams = [
            'OpenAgents Development',
            'Marketing Team',
            'Customer Support',
            'Product Management',
            'Design Team'
        ];

        foreach ($teams as $teamName) {
            $team = Team::factory()->create(['name' => $teamName]);
            $user->teams()->attach($team);

            // Create projects for each team
            $projects = [
                'Website Redesign',
                'Mobile App Development',
                'Customer Feedback Analysis',
                'New Feature Implementation',
                'Performance Optimization'
            ];

            foreach ($projects as $projectName) {
                $project = Project::factory()->create([
                    'name' => $projectName . ' - ' . $teamName,
                    'team_id' => $team->id
                ]);

                // Create threads for each project
                for ($i = 1; $i <= 3; $i++) {
                    $thread = Thread::factory()->create([
                        'project_id' => $project->id,
                        'user_id' => $user->id,
                        'title' => "Thread $i - $projectName - $teamName",
                    ]);

                    Message::factory(5)->create([
                        'thread_id' => $thread->id,
                        'user_id' => $user->id,
                    ]);
                }
            }
        }

        // Create some personal threads for the user
        for ($i = 1; $i <= 2; $i++) {
            $thread = Thread::factory()->create([
                'user_id' => $user->id,
                'project_id' => null,
                'title' => "Personal Thread $i",
            ]);

            Message::factory(3)->create([
                'thread_id' => $thread->id,
                'user_id' => $user->id,
            ]);
        }

        // Output a message to confirm the seeding was successful
        $this->command->info('Teams, projects, threads, and messages have been added to the user with id 1.');
    }
}