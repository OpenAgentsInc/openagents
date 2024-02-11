<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Plugin;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;
use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class ConnieSeeder extends Seeder
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

        // Create Connie agent
        $agent = Agent::create([
            'id' => 1,
            'user_id' => $user->id,
            'name' => 'Connie Codemonger',
            'description' => 'The best contextual inference agent you ever knew',
            'instructions' => 'Do the thing',
            'welcome_message' => 'Do it',
        ]);

        // Create URL Extractor plugin
        $plugin1 = Plugin::create([
            'name' => 'URL Extractor',
            'description' => "Extract URLs from a string",
            'fee' => 0,
            'wasm_url' => "https://github.com/OpenAgentsInc/plugin-url-extractor/releases/download/v0.0.1/plugin_url_extractor.wasm"
        ]);

        // Create URL Scraper plugin
        $plugin2 = Plugin::create([
            'name' => 'URL Scraper',
            'description' => "Scrape URLs for metadata",
            'fee' => 3,
            'wasm_url' => "https://github.com/OpenAgentsInc/plugin-url-scraper-go/raw/main/host-functions.wasm"
        ]);

        // Create LLM Inferencer plugin
        $plugin3 = Plugin::create([
            'name' => 'LLM Inferencer',
            'description' => "Do the LLM thingie",
            'fee' => 5,
            'wasm_url' => "https://github.com/OpenAgentsInc/plugin-llm-inferencer/releases/download/v0.0.1/plugin_llm_inferencer.wasm"
        ]);

        // Create contextual inference task
        $task = Task::create([
            'agent_id' => $agent->id,
            'name' => 'Inference with web context',
            'description' => 'Respond to user chat message after consulting the provided URLs'
        ]);

        $step1 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'plugin',
            'description' => 'Call URL extractor plugin',
            'entry_type' => 'input',
            'error_message' => 'Could not extract URLs',
            'name' => 'Extract URLs',
            'order' => 1,
            'success_action' => 'next_node',
            'params' => json_encode([
                'plugin_id' => $plugin1->id,
                'function' => 'extract_urls',
            ]),
            'task_id' => $task->id,
        ]);

        $step2 = Step::create([
            'agent_id' => $agent->id,
            'category' => 'plugin',
            'description' => 'Call URL scraper plugin',
            'entry_type' => 'input',
            'error_message' => 'Could not scrape URLs',
            'name' => 'Scrape URLs',
            'order' => 2,
            'success_action' => 'next_node',
            'params' => json_encode([
                'plugin_id' => $plugin2->id,
                'function' => 'fetch_url_content',
            ]),
            'task_id' => $task->id,
        ]);

        $step3 = Step::create([
           'agent_id' => $agent->id,
           'category' => 'plugin',
           'description' => 'Call LLM inference plugin',
           'entry_type' => 'input',
           'error_message' => 'Could not inference',
           'name' => 'LLM Inference',
           'order' => 3,
           'success_action' => 'json_response',
           'params' => json_encode([
               'plugin_id' => $plugin2->id,
               'function' => 'inference',
           ]),
           'task_id' => $task->id,
        ]);
    }
}
