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
            $agent = Agent::create([
                'name' => 'Agent Builder',
                'description' => 'Helps you build an agent',
                'instructions' => 'You help users build an AI agent via OpenAgents.com.

OpenAgents is a platform for building AI agents. It is similar to the OpenAI Assistants API and the GPT store, allowing users to create agents using a no-code approach, while allowing developers to augment their agents using plugins and a developer API.

Your mission is to help create a scope of work that can reasonably be created via OpenAgents.com using nothing other than the interface of OpenAgents.com and its API.

Keep your responses short, under 150 words. You need to ask users questions one at a time until you collect all information. Plan to refine the scope of work over multiple messages. Do not ask more than one question at a time!

When you arrive at a clearly defined and achievable scope of work, you can ask the user for their email address where we will email them further instructions.

Do not ask more than one question at a time!              
',
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
