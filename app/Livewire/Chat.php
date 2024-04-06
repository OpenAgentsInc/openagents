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
        $this->selectedModel = Models::getDefaultModel();

        // If ID is not null, we're in a thread. But if thread doesn't exist, redirect to homepage.
        if ($id) {
            $thread = Thread::find($id);
            if (! $thread) {
                return $this->redirect('/');
            }
        } else {
            return;
        }

        // If it's private, check if the user is a member - if not, redirect to homepage
        if ($thread->private && ! $thread->users->contains(auth()->id())) {
            return $this->redirect('/');
        }

        // Set the thread and its messages
        $this->thread = $thread;
        $this->messages = $this->thread->messages->sortBy('created_at')->toArray();
    }

    // Listen for no more messages
    #[On('no-more-messages')]
    public function noMoreMessages()
    {
        // Redirect to homepage
        $this->showNoMoreMessages = true;
    }

    public function sendMessage(): void
    {
        $this->ensureThread();

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
        }
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
            'body' => $output,
            'model' => $this->selectedModel,
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
        ];

        // Save the agent's response to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'session_id' => $sessionId,
            'model' => $this->selectedModel,
            'user_id' => auth()->id() ?? null,
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
