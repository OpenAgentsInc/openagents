<?php

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Thread;
use Livewire\Component;

class Chat extends Component
{
    // User input from chat form
    public $message_input = '';

    public Agent $agent;

    public Thread $thread;

    public $threads = [];

    public $messages = [];

    public $pending = false;

    private $input = '';

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

        $this->js('$wire.startRun()');
    }

    public function startRun()
    {
        dd($this->input);
    }

    public function render()
    {
        return view('livewire.chat');
    }
}
