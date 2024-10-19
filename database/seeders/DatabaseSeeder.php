<?php

namespace Database\Seeders;

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    /**
     * Seed the application's database.
     */
    public function run(): void
    {
        // Create a test user
        $user = User::factory()->create([
            'name' => 'Test User',
            'email' => 'test@example.com',
        ]);

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
                Project::factory()->create([
                    'name' => $projectName . ' - ' . $teamName,
                    'team_id' => $team->id
                ]);
            }
        }

        // Create additional users
        User::factory(9)->create()->each(function ($user) use ($teams) {
            // Attach each user to 1-3 random teams
            $user->teams()->attach(Team::inRandomOrder()->take(rand(1, 3))->pluck('id'));
        });
    }
}