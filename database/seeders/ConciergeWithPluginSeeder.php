<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Plugin;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;
use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class ConciergeWithPluginSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        // If there's a User, use that. If not, create a user via factory.
        $user = User::find(1);
        if (!$user) {
            $user = User::factory()->create(['id' => 1]);
        }

        $plugin = Plugin::create([
            'name' => 'Count Vowels',
            'description' => "Count vowels in a string",
            'fee' => 0,
            'wasm_url' => "https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm",
        ]);

        // Create Concierge agent
        $agent = Agent::create([
            'id' => 1,
            'user_id' => $user->id,
            'name' => 'The Concierge',
            'description' => 'A chatbot that responds to user messages after consulting a knowledge base',
            'instructions' => 'Respond to user chat message after consulting knowledge base',
            'welcome_message' => 'Hello, I am the Concierge. How can I help you?',
        ]);

        // Create main chat task
        $task = Task::create([
            'agent_id' => $agent->id,
            'name' => 'Respond With Vowels',
            'description' => 'Respond to user chat message after consulting knowledge base, but only count the vowels of what you would respond with'
        ]);

        // Create the steps
        $step1 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'validation',
            'description' => 'Ensure input is a valid chat message',
            'entry_type' => 'input',
            'error_message' => 'Could not validate input',
            'name' => 'Validate Input',
            'order' => 1,
            'success_action' => 'next_node',
            'task_id' => $task->id,
        ]);

        $step2 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'embedding',
            'description' => 'Convert input to vector embedding',
            'entry_type' => 'node',
            'error_message' => 'Could not generate embedding',
            'name' => 'Embed Input',
            'order' => 2,
            'success_action' => 'next_node',
            'task_id' => $task->id,
        ]);

        $step3 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'similarity_search',
            'description' => 'Compare input to knowledge base',
            'entry_type' => 'node',
            'error_message' => 'Could not run similarity search',
            'name' => 'Similarity Search',
            'order' => 3,
            'success_action' => 'next_node',
            'task_id' => $task->id,
        ]);

        $step4 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'inference',
            'description' => 'Call to LLM to generate response',
            'entry_type' => 'node',
            'error_message' => 'Could not call to LLM',
            'name' => 'Call LLM',
            'order' => 4,
            'success_action' => 'next_node',
            'task_id' => $task->id,
        ]);

        $step5 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'plugin',
            'description' => 'Call vowels plugin to count vowels in input',
            'entry_type' => 'node',
            'error_message' => 'Could not count vowels',
            'name' => 'Count Vowels',
            'order' => 5,
            'success_action' => 'json_response',
            'params' => json_encode([
                'plugin_id' => $plugin->id,
                'function' => 'count_vowels',
            ]),
            'task_id' => $task->id,
        ]);
    }
}
