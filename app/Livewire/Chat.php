<?php

namespace App\Livewire;

use App\Models\Agent;
use App\Models\Task;
use App\Models\Thread;
use App\Services\Inferencer;
use Exception;
use Illuminate\Contracts\Foundation\Application;
use Illuminate\Contracts\View\Factory;
use Illuminate\View\View;
use Livewire\Component;
use Livewire\WithFileUploads;

class Chat extends Component
{
    use WithFileUploads;

    public $images = [];

    public $body = '';

    public $input = '';

    public Agent $agent;

    public Thread $thread;

    public $threads = [];

    public $messages = [];

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
        $this->input = $this->body;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'sender' => 'user',
        ];

        // Clear the input
        $this->body = '';
        $this->pending = true;

        $this->js('$wire.runTask()');
    }

    public function runTask(): void
    {
        $messageContent = '';

        $logFunction = function ($message) {
            $this->stream(
                to: 'taskProgress',
                content: "Executing step: $message <br />"
            );
        };

        $streamFunction = function ($response) use (&$messageContent) {
            $token = $response['choices'][0]['delta']['content'] ?? '';
            $this->stream(
                to: 'streamtext',
                content: $token
            );
            $messageContent .= $token;
        };

        $output = $this->routeInput($this->input, $logFunction, $streamFunction);

        // worst code in the world
        if (empty($messageContent)) {
            try {
                // If output is a json blob, decode it
                if (is_string($output) && json_decode($output)) {
                    $output = json_decode($output);
                    // $output = json_decode($output);
                    $messageContent = $output->output;
                } else {
                    $messageContent = $output['output'];
                }

            } catch (Exception $e) {
                dd($output);
                dd($e->getMessage());
            }

        }

        // $task = Task::where('name', 'Inference with web context')->firstOrFail();

        // $output = $task->agent->runTask($task, [
        //     'input' => $this->input,
        // ], $logFunction, $streamFunction);

        // Append the response to the chat
        $this->messages[] = [
            'body' => $messageContent,
            'sender' => 'agent',
        ];

        $this->pending = false;
    }

    private function routeInput($input, $logFunction, $streamFunction)
    {
        // Does input contain a URL anywhere inside it?
        $containsUrl = preg_match('/\b(?:https?|ftp):\/\/\S+\b/', $input);

        // If yes, run the "Inference with web context" task
        if ($containsUrl) {
            $task = Task::where('name', 'Inference with web context')->firstOrFail();

            $output = $task->agent->runTask([
                'input' => $input,
            ], $task, $this->thread, $logFunction, $streamFunction);

        } else {
            $this->thread->messages()->create([
                //                'user_id' => auth()->id(),
                'body' => $input,
                //                'sender' => 'user',
            ]);
            $output = Inferencer::llmInference(['input' => $input], $this->thread, $this->agent, $streamFunction);
            $this->thread->messages()->create([
                //                'user_id' => auth()->id(),
                'body' => $output['output'],
                //                'sender' => 'agent',
            ]);
        }

        return $output;
    }

    public function render(): \Illuminate\Contracts\View\View|\Illuminate\Foundation\Application|Factory|View|Application
    {
        return view('livewire.chat');
    }
}
