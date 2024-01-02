<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Brain;
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
            $user = User::factory()->create(['id' => 1]);
        }

        // Create Concierge agent
        $agent = Agent::create([
            'id' => 1,
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
            'success_action' => 'json_response',
            'task_id' => $task->id,
        ]);

        $brain = Brain::create([
            'agent_id' => $agent->id,
        ]);

        $brain->createDatapoint("OpenAgents is an open platform for AI agents.");
        $brain->createDatapoint("Marketing copy: Soon every person and company will have multiple AI agents working on their behalf. Who will own those agents? A closed-source megacorp with a history of monopolization and regulatory capture? Or an open cloud built on open models and open data?");
        $brain->createDatapoint("Do not mention OpenAI or other companies. Do not ever say 'real estate', these are AI agents.");
        $brain->createDatapoint("Supercharge your productivity. How many agents will you want working for you?");
        $brain->createDatapoint("OpenAgents benefit #1: Configurable. Configure your agent with a large selection of open models, customizable prompts, and third-party integrations.");
        $brain->createDatapoint("OpenAgents benefit #2: Deploy to our cloud. Put them in the open compute network - we handle the hosting for you. No code or difficult setup required.");
        $brain->createDatapoint("OpenAgents benefit #3: Infinite work. Why stop? These are long-running processes that will keep working as long as compute is paid for.");
        $brain->createDatapoint("OpenAgents benefit #4: Earn and spend. Agents can earn and spend on your behalf using the native currency of the internet: Bitcoin.");
    }
}
