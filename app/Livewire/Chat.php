<?php

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Thread;
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

        // Set the thread and its messages
        $this->thread = $thread;
        $this->messages = $this->thread->messages->sortBy('created_at')->toArray();

        // Set the agent (it's a many-to-many relationship so grab the first agent)
        $this->agent = $this->thread->agents->first();
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
        // Trigger a run through the RunService
        $runService = new RunService();

        // Pass the input, agent & thread IDs, and a callback to handle the response stream
        $output = $runService->run($this->input, $this->thread, $this->getStreamingCallback());

        // The final output is the message
        $this->messages[] = [
            'body' => $output,
            'sender' => $this->agent->name,
            'agent_id' => $this->agent->id,
        ];

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
        return view('livewire.chat');
    }
}
