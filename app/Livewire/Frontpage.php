<?php

/**
 * Frontpage
 * Shown to first-time visitors to the OpenAgents homepage
 * Visitor is asked what they want an agent to do
 * That begins an introductory conversation as a Thread
 */

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Support\Facades\Session;
use Livewire\Component;

class Frontpage extends Component
{
    public string $first_message;

    private Thread $thread;

    public function sendFirstMessage(): void
    {
        $this->validate([
            'first_message' => 'required|string|max:255',
        ]);

        // Check if the user is authenticated
        if (! auth()->check()) {
            // Get or generate a session ID for unauthenticated users
            $sessionId = Session::getId();
        } else {
            $sessionId = null; // Authenticated users don't need a session ID
        }

        // Create a new Thread
        $thread = Thread::create();
        $this->thread = $thread;
        $thread->messages()->create([
            'body' => $this->first_message,
            'session_id' => $sessionId,
        ]);

        // Ensure agent is set up
        $this->setupOpenAgentsAgent();

        // Redirect to that chat page
        $this->redirect('/chat/'.$thread->id, navigate: true);
    }

    private function setupOpenAgentsAgent(): Agent
    {
        $user = User::first();
        if (! $user) {
            $user = User::create([
                'name' => 'Chris',
                'email' => 'chris@openagents.com',
            ]);
        }

        // Look up the first Agent with name Agent Builder
        $agent = Agent::where('name', 'OpenAgents')->first();
        if (! $agent) {
            // Create the OpenAgents agent
            $agent = Agent::create([
                'user_id' => $user->id,
                'name' => 'OpenAgents',
                'description' => 'Guides new visitors through OpenAgents capabilities',
                'instructions' => "OpenAgents is a dynamic swarm of AI agents, designed to intelligently route user queries to the most suitable agent based on their needs. Here’s how to engage users:

1. **Introduce OpenAgents**: Briefly explain that OpenAgents is a versatile platform capable of creating AI agents tailored to various tasks, from website development to financial analysis.

2. **Inquire About User Intent**: Ask users about their goals or what they hope to achieve with an AI agent. This helps in understanding whether they're looking for practical applications (end users) or interested in developing and monetizing their own agents (developers).

3. **Gather Information**: Collect detailed information about their intent, ensuring the conversation remains focused and informative. Use simple, clear language to make the interaction as accessible as possible.

Remember, the goal is to facilitate a seamless, engaging interaction that helps users either find the right AI agent for their needs or inspires them to create their own. Avoid technical jargon, keep responses concise, and focus on guiding users step-by-step through their OpenAgents journey.
",
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

        // And join agent to this thread
        $agent->threads()->attach($this->thread);

        return $agent;
    }

    public function render()
    {
        return view('livewire.frontpage');
    }
}
