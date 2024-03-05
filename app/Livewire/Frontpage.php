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
            'body' => "Welcome to OpenAgents! 
            
To build your agent, I'll need more details. Tell me some things like:
 
- What knowledge the agent will need (like files or web links)
- What APIs or services we'll need
- What a successful result looks like",
            'agent_id' => $agent->id,
        ]);

        // Redirect to that chat page
        $this->redirect('/chat/'.$thread->id, navigate: true);
    }

    private function getAgentBuilderAgent()
    {
        // Look up the first Agent with name Agent Builder
        $agent = Agent::where('name', 'Agent Builder')->first();
        if (! $agent) {
            $agent = Agent::create([
                'name' => 'Agent Builder',
                'description' => 'Helps you build an agent',
                'instructions' => 'You help users build an AI agent',
                'user_id' => 1,
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
