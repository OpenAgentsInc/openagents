<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
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
                'prompt' => 'Make a pull request that solves a GitHub issue',
            ]);

            // For each agent, create two runs
            $run1 = Run::factory()->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
                'amount' => 0.01,
                'description' => 'MarketerAgent tried to send a tweet',
                'status' => 'failed',
                'output' => json_encode(['huge error because lol']),
            ]);

            // For each run, create steps
            Step::factory(5)->create([
                'agent_id' => $agent->id,
                'run_id' => $run1->id,
            ]);

            $run2 = Run::factory()->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
                'amount' => 0.02,
                'description' => 'GitHubAgent opened pull request',
                'status' => 'success',
                'output' => json_encode(['response' => 'heyyyyyy lol that work']),
            ]);

            Step::factory(5)->create([
                'agent_id' => $agent->id,
                'run_id' => $run2->id,
            ]);
        });
    }
}
