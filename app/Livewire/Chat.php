<?php

namespace App\Livewire;

use App\Models\Task;
use Livewire\Component;

class Chat extends Component
{
    public $body = '';
    public $messages = [];

    public function sendMessage()
    {
        $task = Task::where('name', 'Inference with web context')->firstOrFail();
        $input = $this->body;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $input,
            'from' => 'You',
        ];

        // Clear the input
        $this->body = '';

        $output = $task->agent->runTask($task, [
            'input' => $input,
        ]);

        // Append the response to the chat
        $this->messages[] = [
            'body' => $output,
            'from' => $task->agent->name,
        ];
    }

    public function render()
    {
        return view('livewire.chat')->layout('components.layouts.chat');
    }
}
