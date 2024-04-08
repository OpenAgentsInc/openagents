<?php

namespace App\Livewire;

use App\AI\Models;
use App\AI\SimpleInferencer;
use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class Chat extends Component
{
    // Whether to show the "no more messages" message
    public $showNoMoreMessages = false;

    public $waitingForStream = false;

    // User input from chat form
    public $message_input = '';

    // The saved input
    public $input = '';

    // The thread we're chatting in
    public Thread $thread;

    // The messages we render on the page
    public $messages = [];

    // Whether we're waiting for a response
    public $pending = false;

    public $selectedModel = '';

    // Listen to select-model event
    #[On('select-model')]
    public function selectModel($model)
    {
        $this->selectedModel = $model;
    }

    public function mount($id = null)
    {
        // Set the default model
        $this->selectedModel = Models::getDefaultModel();

        // If ID is not null, we're in a thread. But if thread doesn't exist, redirect to homepage.
        if ($id) {
            $thread = Thread::find($id);
            // If there's no thread or thread doesn't belong to the user and doesn't match the session ID, redirect to homepage
            if (! $thread || ($thread->user_id !== auth()->id()) && ($thread->session_id !== session()->getId())) {
                return $this->redirect('/');
            } else {
                // Notify the sidebar component of the active thread
                $this->dispatch('active-thread', $id);
            }
        } else {
            $this->ensureThread();

            return;
        }

        // Set the thread and its messages
        $this->thread = $thread;
        $this->messages = $this->thread->messages->sortBy('created_at')->toArray();
    }

    // Listen for no more messages

    private function ensureThread()
    {
        if (empty($this->thread)) {
            // Create a new Thread
            $data = [
                'title' => 'New chat',
                'session_id' => auth()->check() ? null : Session::getId(),
            ];

            if (auth()->check()) {
                $data['user_id'] = auth()->id();
            }

            $thread = Thread::create($data);
            $this->thread = $thread;
            $this->dispatch('thread-update');

            // If the current chat URL is not chat/{thread_id}, redirect to the correct URL
            //            if (request()->path() !== 'chat/'.$this->thread->id) {
            return $this->redirect('/chat/'.$this->thread->id, true);
            //            }
        } else {
            dd('what');
        }
    }

    #[On('no-more-messages')]
    public function noMoreMessages()
    {
        // Redirect to homepage
        $this->showNoMoreMessages = true;
    }

    public function sendMessage(): void
    {
        // Save this input even after we clear the form this variable is tied to
        $this->input = $this->message_input;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'sender' => 'You',
            'user_id' => auth()->id(), // Add user_id if logged in
            'session_id' => auth()->check() ? null : Session::getId(), // Add session_id if not logged in
        ];

        // Clear the input
        $this->message_input = '';
        $this->pending = true;

        // Call simpleRun after the next render
        $this->js('$wire.simpleRun()');
    }

    public function simpleRun()
    {
        // This method skips node flow and directly processes the response

        // Authenticate user session or proceed without it
        $sessionId = auth()->check() ? null : Session::getId();

        // Save user message to the thread
        $this->thread->messages()->create([
            'body' => $this->input,
            'session_id' => $sessionId,
            'user_id' => auth()->id() ?? null,
        ]);

        // Simply do it
        $output = SimpleInferencer::inference($this->input, $this->selectedModel, $this->thread, $this->getStreamingCallback());

        // Append the response to the chat
        $this->messages[] = [
            'body' => $output['content'],
            'model' => $this->selectedModel,
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
        ];

        // Save the agent's response to the thread
        $this->thread->messages()->create([
            'body' => $output['content'],
            'session_id' => $sessionId,
            'model' => $this->selectedModel,
            'user_id' => auth()->id() ?? null,
            // here add the input and output tokens
        ]);

        // Reset pending status and scroll to the latest message
        $this->pending = false;

        // Optionally notify other components of the new message
        $this->dispatch('message-created');
    }

    private function getStreamingCallback()
    {
        return function ($response) {
            $token = $response['choices'][0]['delta']['content'] ?? '';
            $this->stream(
                to: 'streamtext',
                content: $token
            );
        };
    }

    public function render()
    {
        return view('livewire.chat');
    }
}
