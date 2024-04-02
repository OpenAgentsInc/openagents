<?php

namespace App\Livewire;

use App\AI\SimpleInferencer;
use App\Models\Agent;
use App\Models\Thread;
use App\Services\RunService;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class Chat extends Component
{
    // Whether to show the "no more messages" message
    public $showNoMoreMessages = false;

    // User input from chat form
    public $message_input = '';

    // The saved input we pass to agent
    public $input = '';

    // The agent we're chatting with
    public Agent $agent;

    // The thread we're chatting in
    public Thread $thread;

    // The messages we render on the page
    public $messages = [];

    // Whether we're waiting for a response from the agent
    public $pending = false;

    public $selectedModel = 'mixtral-8x7b-32768';

    // Listen to select-model event
    #[On('select-model')]
    public function selectModel($model)
    {
        $this->selectedModel = $model;
    }

    public function mount($id = null)
    {
        // For now if there's no id, redirect to homepage
        //        if (! $id) {
        //            return $this->redirect('/');
        //        }

        // Find this thread
        $thread = Thread::find($id);
        if (! $thread) {
            return;
        }

        // If it doesn't exist, redirect to homepage
        //        if (! $thread) {
        //            return $this->redirect('/');
        //        }

        // If it's private, check if the user is a member - if not, redirect to homepage
        //        if ($thread->private && ! $thread->users->contains(auth()->id())) {
        //            return $this->redirect('/');
        //        }

        // Set the thread and its messages
        $this->thread = $thread;
        $this->messages = $this->thread->messages->sortBy('created_at')->toArray();

        // Set the agent (it's a many-to-many relationship so grab the first agent)
        $this->agent = $this->thread->agents->first();

        // If the thread was just created, send the first message from the agent
        if (count($this->messages) <= 1) {
            $this->pending = true;
            $this->input = $this->messages[0]['body'] ?? '';
            $this->js('$wire.runFirst()');
        }
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
            'agent_id' => null,
            'sender' => 'You',
        ];

        // Clear the input
        $this->message_input = '';
        $this->pending = true;

        // Call startRun after the next render
        $this->js('$wire.simpleRun()');
        //        $this->js('$wire.startRun()');
    }

    // Example simple response generator

    private function ensureThread()
    {
        if (empty($this->thread)) {
            // Create a new Thread
            $thread = Thread::create();
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
            'agent_id' => null, // Null for user messages
            'session_id' => $sessionId,
        ]);

        // Simply do it
        $output = SimpleInferencer::inference($this->input, $this->selectedModel, $this->thread, $this->getStreamingCallback());

        // Append the response to the chat
        $this->messages[] = [
            'body' => $output,
            'sender' => 'Agent', // $this->agent->name,
            'agent_id' => null, // $this->agent->id,
        ];

        // Save the agent's response to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'session_id' => $sessionId, // or if authed?
            'model' => $this->selectedModel, // 'mixtral-8x7b-32768
            //            'agent_id' => 99, // $this->agent->id, // The agent's ID for their messages
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

    public function startRun()
    {
        // Check if the user is authenticated
        if (! auth()->check()) {
            // Get or generate a session ID for unauthenticated users
            $sessionId = Session::getId();
        } else {
            $sessionId = null; // Authenticated users don't need a session ID
        }

        // Save this user message to the thread
        $this->thread->messages()->create([
            'body' => $this->input,
            'agent_id' => null,
            'session_id' => $sessionId,
        ]);

        // Trigger a run through the RunService
        $runService = new RunService();

        // Pass the input, agent/flow/thread, and a callback to handle the response stream
        $output = $runService->triggerRun([
            'agent' => $this->agent,
            'flow' => $this->agent->flows->first(),
            'thread' => $this->thread,
            'input' => $this->input,
            'streamingFunction' => $this->getStreamingCallback(),
        ]);

        // The final output is the message
        $this->messages[] = [
            'body' => $output,
            'sender' => $this->agent->name,
            'agent_id' => $this->agent->id,
        ];

        // Save the agent message to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'agent_id' => $this->agent->id,
        ]);

        // Reset/scroll
        $this->pending = false;

        // Notify other component we got a message back
        $this->dispatch('message-created');
    }

    public function render()
    {
        return view('livewire.chat');
    }

    public function runFirst()
    {
        // Trigger a run through the RunService
        $runService = new RunService();

        // Pass the input, agent/flow/thread, and a callback to handle the response stream
        $output = $runService->triggerRun([
            'agent' => $this->agent,
            'flow' => $this->agent->flows->first(),
            'thread' => $this->thread,
            'input' => $this->input,
            'streamingFunction' => $this->getStreamingCallback(),
        ]);

        // The final output is the message
        $this->messages[] = [
            'body' => $output,
            'sender' => $this->agent->name,
            'agent_id' => $this->agent->id,
        ];

        // Save the agent message to the thread
        $this->thread->messages()->create([
            'body' => $output,
            'agent_id' => $this->agent->id,
        ]);

        // Reset/scroll
        $this->pending = false;
    }

    private function generateSimpleResponse($input)
    {
        // This is a placeholder for your response generation logic.
        // For now, it just echoes back the input with a prefix.
        return 'Echo: '.$input;
    }
}
