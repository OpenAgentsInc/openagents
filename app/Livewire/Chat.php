<?php

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Thread;
use App\Services\RunService;
use Illuminate\Support\Facades\Session;
use Livewire\Component;

class Chat extends Component
{
    // User input from chat form
    public $message_input = '';

    // The saved input we pass to agent
    public $input = '';

    // The agent we're chatting with
    public Agent $agent;

    // The thread we're chatting in
    public Thread $thread;

    // The messages we render on the page
    public $messages = [];

    // Whether we're waiting for a response from the agent
    public $pending = false;

    public function mount($id = null)
    {
        // For now if there's no id, redirect to homepage
        if (! $id) {
            return $this->redirect('/');
        }

        // Find this thread
        $thread = Thread::find($id);

        // If it doesn't exist, redirect to homepage
        if (! $thread) {
            return $this->redirect('/');
        }

        // If it's private, check if the user is a member - if not, redirect to homepage
        if ($thread->private && ! $thread->users->contains(auth()->id())) {
            return $this->redirect('/');
        }

        // Set the thread and its messages
        $this->thread = $thread;
        $this->messages = $this->thread->messages->sortBy('created_at')->toArray();

        // Set the agent (it's a many-to-many relationship so grab the first agent)
        $this->agent = $this->thread->agents->first();

        // If the thread was just created, send the first message from the agent
        if (count($this->messages) <= 1) {
            $this->pending = true;
            $this->input = $this->messages[0]['body'] ?? '';
            $this->js('$wire.runFirst()');
        }
    }

    public function sendMessage(): void
    {
        // Save this input even after we clear the form this variable is tied to
        $this->input = $this->message_input;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'agent_id' => null,
            'sender' => 'You',
        ];
        $this->dispatch('scrollToBottomAgain');

        // Clear the input
        $this->message_input = '';
        $this->pending = true;

        // Call startRun after the next render
        $this->js('$wire.startRun()');
    }

    public function startRun()
    {
        // Check if the user is authenticated
        if (! auth()->check()) {
            // Get or generate a session ID for unauthenticated users
            $sessionId = Session::getId();
        } else {
            $sessionId = null; // Authenticated users don't need a session ID
        }

        // Save this user message to the thread
        $this->thread->messages()->create([
            'body' => $this->input,
            'agent_id' => null,
            'session_id' => $sessionId,
        ]);

        // Trigger a run through the RunService
        $runService = new RunService();

        // Pass the input, agent/flow/thread, and a callback to handle the response stream
        $output = $runService->triggerRun([
            'agent' => $this->agent,
            'flow' => $this->agent->flows->first(),
            'thread' => $this->thread,
            'input' => $this->input,
            'streamingFunction' => $this->getStreamingCallback(),
        ]);

        // The final output is the message
        $this->messages[] = [
            'body' => $output,
            'sender' => $this->agent->name,
            'agent_id' => $this->agent->id,
        ];

        // Save the agent message to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'agent_id' => $this->agent->id,
        ]);

        // Reset/scroll
        $this->pending = false;
        $this->dispatch('scrollToBottomAgain');
    }

    private function getStreamingCallback()
    {
        return function ($response) {
            $token = $response['choices'][0]['delta']['content'] ?? '';
            $this->stream(
                to: 'streamtext',
                content: $token
            );
            $this->dispatch('scrollToBottomAgain');
        };
    }

    public function render()
    {
        return view('livewire.chat')
            ->layout('components.layouts.new');
    }

    public function runFirst()
    {
        // Trigger a run through the RunService
        $runService = new RunService();

        // Pass the input, agent/flow/thread, and a callback to handle the response stream
        $output = $runService->triggerRun([
            'agent' => $this->agent,
            'flow' => $this->agent->flows->first(),
            'thread' => $this->thread,
            'input' => $this->input,
            'streamingFunction' => $this->getStreamingCallback(),
        ]);

        // The final output is the message
        $this->messages[] = [
            'body' => $output,
            'sender' => $this->agent->name,
            'agent_id' => $this->agent->id,
        ];

        // Save the agent message to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'agent_id' => $this->agent->id,
        ]);

        // Reset/scroll
        $this->pending = false;
        $this->dispatch('scrollToBottomAgain');
    }
}
