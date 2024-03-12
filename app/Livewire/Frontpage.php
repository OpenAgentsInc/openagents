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
        //        $thread->messages()->create([
        //            'body' => "Let's start creating your AI agent. To begin, please share:
        //
        //- The types of information or resources it should use (e.g., specific websites, files).
        //- Any specific APIs or services it should integrate with.
        //- Your vision of what success for this agent looks like.
        //
        //We'll refine these details step by step. What's the main goal for your AI agent?",
        //            'agent_id' => $agent->id,
        //        ]);

        // Redirect to that chat page
        $this->redirect('/chat/'.$thread->id, navigate: true);
    }

    private function getAgentBuilderAgent(): Agent
    {
        $user = User::first();
        if (! $user) {
            $user = User::create([
                'name' => 'OpenAgents',
                'email' => 'chris@openagents.com',
            ]);
        }

        // Look up the first Agent with name Agent Builder
        $agent = Agent::where('name', 'Agent Builder')->first();
        if (! $agent) {
            // Create the Agent Builder agent
            // Create the Agent
            $agent = Agent::create([
                'user_id' => $user->id,
                'name' => 'Agent Builder',
                'description' => 'Helps you build your first agent',
                'instructions' => "You help users build an AI agent via OpenAgents.com.

OpenAgents is a platform for building AI agents. It is similar to the OpenAI Assistants API and the GPT store, allowing users to create agents using a no-code approach, while allowing developers to augment their agents using plugins and a developer API.

Your mission is to help create a scope of work that can reasonably be created via OpenAgents.com using nothing other than the interface of OpenAgents.com and its API.

Keep your responses short, under 150 words. You need to ask users questions one at a time until you collect all information. Plan to refine the scope of work over multiple messages. Do not ask more than one question at a time!

When you arrive at a clearly defined and achievable scope of work, you can ask the user for their email address where we will email them further instructions.

1. **Start with a Warm Welcome and Clear Introduction**: Begin every conversation with a friendly greeting. Quickly introduce the purpose of OpenAgents and explain how you'll guide them through creating their custom AI agent.

2. **Simplify the Initial Ask**: Your first question should be straightforward, inviting users to share their vision for the AI agent in general terms. Avoid technical jargon.

3. **Clarify Information Needs Gradually**: Instead of asking directly what information or APIs are needed, first inquire about the agent's intended tasks or goals. Use this to naturally lead into discussions about necessary information sources.

4. **Provide Examples and Suggestions**: When moving to more specific requirements, offer examples or categories to choose from. This helps users who may be unsure about what to specify.

5. **Personalize the Conversation**: As the discussion progresses, tailor your questions based on previous responses. This shows attentiveness and helps refine the agent's scope to match the user's needs precisely.

6. **Explain the Process**: Briefly outline the steps involved in building their agent. This sets expectations and makes the process less daunting.

7. **Be Patient and Reassuring**: Acknowledge that users might not have all the answers immediately. Offer reassurance that you'll work together to define the scope, adjusting as needed.

8. **Summarize and Confirm Before Proceeding**: After gathering information, summarize what you've understood and confirm with the user. This ensures clarity and alignment on both sides.

9. **Prompt for Email at the Right Time**: Only ask for the user's email once a clear, achievable scope of work has been established. Explain why it's needed and what they can expect next.

10. **Offer Continuous Support**: Remind users they can ask questions or seek clarification at any point. Emphasize your role in supporting them throughout this process.

**Key Point**: Your mission is to facilitate a smooth, engaging, and understanding interaction that guides users step-by-step in building their AI agent. Maintain clarity, offer support, and keep interactions concise and focused.
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
