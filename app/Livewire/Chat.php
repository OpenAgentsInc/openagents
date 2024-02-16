<?php

namespace App\Livewire;

use App\Models\Task;
use Livewire\Component;
use Livewire\Attributes\On;

class Chat extends Component
{
    public $body = '';
    public $input = '';
    public $messages = [];

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
        $logFunction = function($message) {
            $this->stream(
                to: 'taskProgress',
                content: "Executing step: $message <br />"
            );
        };
        $task = Task::where('name', 'Inference with web context')->firstOrFail();
        $output = $task->agent->runTask($task, [
            'input' => $this->input,
        ], $logFunction);
        // Decode the JSON response to extract the message content
         $decodedOutput = json_decode($output, true);
        // decode again
        $decodedOutput = json_decode($decodedOutput, true);
        $messageContent = $decodedOutput['choices'][0]['message']['content'] ?? 'Response not available';

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
