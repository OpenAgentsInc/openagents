<?php

namespace App\Livewire;

use App\AI\Models;
use App\AI\SimpleInferencer;
use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;
use Livewire\WithFileUploads;

class Chat extends Component
{
    use WithFileUploads;

    public $images = [];

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

    public $selectedAgent = '';

    #[On('select-model')]
    public function selectModel($model)
    {
        $this->selectedModel = $model;
    }

    public function mount($id = null)
    {
        if (request()->query('model')) {
            session()->put('selectedModel', request()->query('model'));
        }

        if (request()->query('agent')) {
            session()->put('selectedAgent', request()->query('agent'));
        }

        // If ID is not null, we're in a thread. But if thread doesn't exist or doesn't belong to the user and doesn't match the session ID, redirect to homepage.
        if ($id) {
            $thread = Thread::find($id);
            if (! $thread || (auth()->check() && $thread->user_id !== auth()->id()) || (! auth()->check() && $thread->session_id !== session()->getId())) {
                return $this->redirect('/', true);
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

        // Set the selected model
        $this->selectedModel = Models::getModelForThread($this->thread);

        // If the selectedAgent session var is set, use it
        if (session()->has('selectedAgent')) {
            $this->selectedAgent = session('selectedAgent');
        }
    }

    private function ensureThread()
    {
        if (empty($this->thread)) {
            // Check if the user or guest has a recent thread with no messages
            $recentThread = null;

            if (auth()->check()) {
                $recentThread = Thread::where('user_id', auth()->id())
                    ->whereDoesntHave('messages')
                    ->latest()
                    ->first();
            } else {
                $recentThread = Thread::where('session_id', Session::getId())
                    ->whereDoesntHave('messages')
                    ->latest()
                    ->first();
            }

            if ($recentThread) {
                $this->thread = $recentThread;
                $this->dispatch('thread-update');

                return $this->redirect('/chat/'.$this->thread->id, true);
            }

            // If no recent thread found, create a new one
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

            return $this->redirect('/chat/'.$this->thread->id, true);
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

        $this->handleImageInput();

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
        $this->dispatch('message-created');
        $this->js('$wire.simpleRun()');
    }

    private function handleImageInput()
    {
        $imageDataArray = [];

        // Handle file upload
        if (! empty($this->images)) {
            foreach ($this->images as $image) {
                // Read the image file contents
                $imageContents = $image->get();

                // Encode the image contents to base64
                $imageBase64 = base64_encode($imageContents);

                // Collect the base64-encoded images
                $imageDataArray[] = $imageBase64;
            }
        }

        dd($imageDataArray);
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
            'input_tokens' => $output['input_tokens'],
            'output_tokens' => $output['output_tokens'],
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
