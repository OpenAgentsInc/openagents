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

        // Create a new Thread
        $thread = Thread::create();
        $this->thread = $thread;
        $thread->messages()->create([
            'body' => $this->first_message,
        ]);

        // Send first message from agent
        $agent = $this->getAgentBuilderAgent();
        $thread->messages()->create([
            'body' => "Let's start creating your AI agent. To begin, please share:

- The types of information or resources it should use (e.g., specific websites, files).
- Any specific APIs or services it should integrate with.
- Your vision of what success for this agent looks like.

We'll refine these details step by step. What's the main goal for your AI agent?",
            'agent_id' => $agent->id,
        ]);

        // Redirect to that chat page
        $this->redirect('/chat/'.$thread->id, navigate: true);
    }

    private function getAgentBuilderAgent(): Agent
    {
        $user = User::first();
        if (! $user) {
            User::create([
                'name' => 'OpenAgents',
                'email' => 'chris@openagents.com',
            ]);
        }

        // Look up the first Agent with name Agent Builder
        $agent = Agent::where('name', 'Agent Builder')->first();
        if (! $agent) {
            // Create the Agent Builder agent
            $agent = Agent::create([
                'name' => 'Agent Builder',
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
