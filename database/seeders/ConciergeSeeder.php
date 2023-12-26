<?php

namespace Database\Seeders;

use App\Models\Step;
use App\Models\Task;
use App\Models\User;
use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class ConciergeSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        // If there's a User, use that. If not, create a user via factory.
        $user = User::find(1);
        if (!$user) {
            $user = User::factory()->create();
        }

        // Create Concierge agent
        $agent = Agent::create([
            'user_id' => $user->id,
            'name' => 'The Concierge',
        ]);

        // Create main chat task
        $task = Task::create([
            'agent_id' => $agent->id,
            'description' => 'Respond to user chat message after consulting knowledge base'
        ]);

        // Create the steps
        $step1 = Step::create([
            'description' => 'Ensure input is a valid chat message',
            'entry_type' => 'input',
            'error_type' => 'Could not validate input',
            'name' => 'Validate Input',
            'order' => 1,
            'success_action' => 'next_node',
            'task_id' => $task->id
        ]);
    }
}
