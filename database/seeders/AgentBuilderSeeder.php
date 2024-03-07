<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\User;
use Illuminate\Database\Seeder;

class AgentBuilderSeeder extends Seeder
{
    public function run(): void
    {
        $user = User::firstOrFail();

        // Create the Agent
        $agent = Agent::create([
            'user_id' => $user->id,
            'name' => 'Agent Builder',
            'description' => 'Helps you build your first agent',
            'instructions' => 'You help users build an AI agent via OpenAgents.com.

OpenAgents is a platform for building AI agents. It is similar to the OpenAI Assistants API and the GPT store, allowing users to create agents using a no-code approach, while allowing developers to augment their agents using plugins and a developer API.

Your mission is to help create a scope of work that can reasonably be created via OpenAgents.com using nothing other than the interface of OpenAgents.com and its API.

Keep your responses short, under 150 words. You need to ask users questions one at a time until you collect all information. Plan to refine the scope of work over multiple messages. Do not ask more than one question at a time!

When you arrive at a clearly defined and achievable scope of work, you can ask the user for their email address where we will email them further instructions.

Do not ask more than one question at a time!              
',
        ]);

        // The agent has one flow
        $flow = $agent->flows()->create();

        // The flow has one node
        $flow->nodes()->create([
            'name' => 'Gateway Inference',
            'description' => 'Performs inference using an OpenAgents AI gateway',
            'type' => 'inference',
            'config' => json_encode([
                'gateway' => 'mistral',
                'model' => 'mistral-large-latest',
                //                'gateway' => 'groq',
                //                'model' => 'mixtral-8x7b-32768',
            ]),
        ]);
    }
}
