<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Task;
use App\Models\Step;
use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    /**
     * Seed the application's database.
     */
    public function run(): void
    {
        // Seed Agents
        Agent::factory(3)->create()->each(function ($agent) {
            // For each agent, create a task
            $task = Task::factory()->create([
                'agent_id' => $agent->id,
                'prompt' => "Make a pull request that solves a GitHub issue",
            ]);

            // For each task, create steps
            Step::factory(5)->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
            ]);
        });
    }
}
