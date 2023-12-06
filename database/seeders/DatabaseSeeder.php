<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
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

            // For each agent, create two runs
            Run::factory()->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
                'amount' => 0.01,
                'description' => "MarketerAgent tried to send a tweet",
                'status' => 'failed',
                'output' => json_encode(['huge error because lol'])
            ]);

            Run::factory()->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
                'amount' => 0.02,
                'description' => "GitHubAgent opened pull request",
                'status' => 'success',
                'output' => json_encode(['response' => 'heyyyyyy lol that work'])
            ]);

            // For each task, create steps
            Step::factory(5)->create([
                'agent_id' => $agent->id,
                'task_id' => $task->id,
            ]);
        });
    }
}
