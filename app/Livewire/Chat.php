<?php

namespace App\Livewire;

use App\Models\Conversation;
use App\Models\Task;
use Livewire\Component;
use Livewire\Attributes\On;

class Chat extends Component
{
    public $body = '';
    public $input = '';
    public $conversations = [];
    public $messages = [];

    public function mount()
    {
        // Load this user's conversations from database
        $this->conversations = Conversation::all();
    }

    public function sendMessage()
    {
        $this->input = $this->body;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'from' => 'You',
        ];

        // Clear the input
        $this->body = '';

        $this->js('$wire.runTask()');
    }

    public function runTask()
    {
        $messageContent = "";
        $logFunction = function($message) {
            $this->stream(
                to: 'taskProgress',
                content: "Executing step: $message <br />"
            );
        };
        $streamFunction = function($response) use (&$messageContent) {
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
