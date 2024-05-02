<?php

namespace App\Livewire;

use App\AI\Models;
use App\AI\NostrInference;
use App\AI\NostrRag;
use App\AI\SimpleInferencer;
use App\Models\Agent;
use App\Models\AgentFile;
use App\Models\NostrJob;
use App\Models\Thread;
use App\Services\ImageService;
use App\Services\NostrService;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;
use Livewire\WithFileUploads;

class Chat extends Component
{
    use WithFileUploads;

    public $images = [];

    public $images_to_upload = [];

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

    #[On('select-agent')]
    public function selectAgent($agent)
    {
        $this->selectedAgent = $agent;
    }

    public function mount($id = null)
    {

        if (request()->query('model')) {
            session()->put('selectedModel', request()->query('model'));
        }
        $agent = Agent::first();
        $this->selectedAgent = $agent ? $agent->id : null;
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
        if (session()->has('agent')) {
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
        $this->images_to_upload = $this->images;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'sender' => 'You',
            'user_id' => auth()->id(), // Add user_id if logged in
            'session_id' => auth()->check() ? null : Session::getId(), // Add session_id if not logged in
            'agent_id' => $this->selectedAgent ?: null,
        ];

        // Clear the input
        $this->message_input = '';
        $this->pending = true;
        $this->images = [];

        // Call simpleRun after the next render
        $this->dispatch('message-created');
        if (! $this->selectedAgent) {
            $this->js('$wire.simpleRun()');
        } else {
            $this->js('$wire.ragRun()');
        }

    }

    public function simpleRun()
    {
        // Convert any images to messages with descriptions generated by vision LLM
        $this->handleImageInput();

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
        $message = [
            'body' => $output['content'],
            'model' => $this->selectedModel,
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
        ];
        $this->messages[] = $message;

        // Save the agent's response to the thread
        $this->thread->messages()->create(array_merge($message, [
            'input_tokens' => $output['input_tokens'],
            'output_tokens' => $output['output_tokens'],
        ]));

        // Reset pending status and scroll to the latest message
        $this->pending = false;

        // Optionally notify other components of the new message
        $this->dispatch('message-created');
    }

    private function handleImageInput()
    {
        if (! empty($this->images_to_upload)) {
            $imageService = new ImageService();
            foreach ($this->images_to_upload as $image) {
                $imageService->addImageToThread($image, $this->thread);
            }
            $this->images_to_upload = [];
        }
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

    public function ragRun()
    {

        try {
            $sessionId = auth()->check() ? null : Session::getId();

            // Save user message to the thread
            $this->thread->messages()->create([
                'body' => $this->input,
                'session_id' => $sessionId,
                'user_id' => auth()->id() ?? null,
                'agent_id' => $this->selectedAgent ?: null,
            ]);

            $nostrRag = new NostrRag(); // Generate history
            $query = $nostrRag->history($this->thread)->summary();

            $documents = AgentFile::where('agent_id', $this->selectedAgent)->pluck('url')->toArray();

            // send to nostra

            $pool = config('services.nostr.pool');

            $job_id = (new NostrService())
                ->poolAddress($pool)
                ->query($query)
                ->documents($documents)
                ->k(1)
                ->maxTokens(2048)
                ->overlap(256)
                ->warmUp(false)
                ->cacheDurationhint('-1')
                ->encryptFor('')
                ->execute();

            // Save to DB
            $nostr_job = new NostrJob();
            $nostr_job->agent_id = $this->selectedAgent;
            $nostr_job->job_id = $job_id;
            $nostr_job->status = 'pending';
            $nostr_job->thread_id = $this->thread->id;
            $nostr_job->save();

        } catch (\Exception $e) {
            Log::error($e);
        }

    }

    #[On('echo:threads.{thread.id},NostrJobReady')]
    public function process_nostr($event)
    {

        $this->selectedModel = 'mistral-small-latest';
        // Authenticate user session or proceed without it
        $sessionId = auth()->check() ? null : Session::getId();

        $job = NostrJob::where('thread_id', $this->thread->id)->find($event['id']);

        // Simply do it
        $output = NostrInference::inference($this->selectedModel, $job, $this->getStreamingCallback());

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

    public function render()
    {
        return view('livewire.chat');
    }
}
