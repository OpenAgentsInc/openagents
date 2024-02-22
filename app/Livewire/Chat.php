<?php

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\Task;
use Livewire\Component;
use Livewire\Attributes\On;

class Chat extends Component
{
    public $body = '';
    public $input = '';
    public $conversation;
    public $conversations = [];
    public $agent;
    public $messages = [];

    public function mount($id = null)
    {
        // If we're in a chat, load the messages
        if ($id) {
            $this->conversation = Conversation::findOrFail($id);
            $this->messages = $this->conversation->messages->toArray();
            $this->agent = $this->conversation->agent;
        }

        // Load this user's conversations from database
        $this->conversations = Conversation::all();
    }

    public function sendMessage()
    {
        // Check if the user is authenticated
        if (!auth()->check()) {
            abort(403, 'Unauthorized action.');
        }

        $this->input = $this->body;

        // If the current conversation is null, create a new one
        if (!$this->conversation) {
            $this->conversation = Conversation::create([
                'title' => 'New Conversation',
                'agent_id' => Agent::first()->id,
            ]);
            $this->conversations = Conversation::all();
        }

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'sender' => 'You',
        ];

        // Clear the input
        $this->body = '';

        $this->js('$wire.runTask()');
    }

    public function runTask()
    {
        $messageContent = "";
        $logFunction = function ($message) {
            $this->stream(
                to: 'taskProgress',
                content: "Executing step: $message <br />"
            );
        };
        $streamFunction = function ($response) use (&$messageContent) {
            $token = $response['choices'][0]['delta']['content'] ?? "";
            $this->stream(
                to: 'streamtext',
                content: $token
            );
            $messageContent .= $token;
        };
        $task = Task::where('name', 'Inference with web context')->firstOrFail();

        $output = $task->agent->runTask($task, [
            'input' => $this->input,
        ], $logFunction, $streamFunction);

        // Append the response to the chat
        $this->messages[] = [
            'body' => $messageContent,
            'from' => $task->agent->name,
        ];
    }

    public function render()
    {
        return view('livewire.chat')->layout('components.layouts.chat');
    }
}
