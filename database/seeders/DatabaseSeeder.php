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
                Project::factory()->create([
                    'name' => $projectName . ' - ' . $teamName,
                    'team_id' => $team->id
                ]);
            }
        }

        // Output a message to confirm the seeding was successful
        $this->command->info('Teams and projects have been added to the user with id 1.');
    }
}