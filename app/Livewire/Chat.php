<?php

namespace App\Livewire;

use App\AI\Models;
use App\AI\PoolInference;
use App\AI\PoolRag;
use App\AI\SimpleInferencer;
use App\Models\Agent;
use App\Models\AgentFile;
use App\Models\Plugin;
use App\Models\PoolJob;
use App\Models\Thread;
use App\Models\User;
use App\Services\ImageService;
use App\Services\LocalLogger;
use App\Services\OpenObserveLogger;
use App\Services\PaymentService;
use App\Utils\PoolUtils;
use Exception;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Session;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\On;
use Livewire\Component;
use Livewire\WithFileUploads;

class Chat extends Component
{
    use LivewireAlert,  WithFileUploads;

    public $images = [];

    public $images_to_upload = [];

    public $showNoMoreMessages = false;

    public $waitingForStream = false;

    public $message_input = '';

    // Whether to show the "no more messages" message
    public $input = '';

    public Thread $thread;

    // User input from chat form
    public $messages = [];

    // The saved input
    public $pending = false;

    public bool $hasSelection = false;

    public $fundLocksIds = [];

    // The thread we're chatting in

    public function mount($id = null)
    {

        // If ID is not null, we're in a thread. But if thread doesn't exist or doesn't belong to the user and doesn't match the session ID, redirect to homepage.
        if ($id) {
            $thread = Thread::find($id);

            if (! $thread) {
                Log::info('Thread not found');

                return $this->redirect('/', true);
            }
            if (auth()->check() && $thread->user_id !== auth()->id()) {

                return $this->redirect('/', true);
            }
            if (! auth()->check() && $thread->session_id !== session()->getId()) {

                return $this->redirect('/', true);
            }

            $this->thread = $thread;

            // Notify the sidebar component of the active thread
            $this->dispatch('active-thread', $id);

        }
        $this->ensureThread();

        if (session()->get('redirecting-with-selection')) {
            $this->hasSelection = true;
            session()->forget('redirecting-with-selection');
        }

        if (request()->query('model')) {
            $this->selectedModel(request()->query('model'));
        }

        if (request()->query('agent')) {
            $this->selectAgent(request()->query('agent'));
        }

        $this->messages = $this->thread->messages()
            ->with('agent') // fetch the agent relationship
            ->orderBy('created_at', 'asc')
            ->get()->toArray();
        // if the campaign_subid exists, pop open a modal
        if (session()->has('campaign_subid')) {
            $this->openModal();
        }
    }

    private function ensureThread()
    {
        if (empty($this->thread) || ! $this->thread) {
            // Check if the user or guest has a recent thread with no messages

            $this->thread = Thread::create([
                'title' => 'New chat',
                'session_id' => auth()->check() ? null : Session::getId(),
                'model' => Models::getDefaultModel(),
                'user_id' => auth()->check() ? auth()->id() : null,
            ]);

            $lastModel = session()->get('lastModel') ?? Models::getDefaultModel();
            $lastAgent = session()->get('lastAgent');
            $this->selectModel($lastModel);
            if ($lastAgent) {
                $this->selectAgent($lastAgent);
            }
            $this->dispatch('thread-update');

            return $this->redirect('/chat/'.$this->thread->id, true);
        }
    }

    public function openModal()
    {
        $this->js('setTimeout(() => { Livewire.dispatch("openModal", { component: "modals.lander-welcome" }) }, 100)');
        // Remove that session variable
        session()->forget('campaign_subid');
    }

    #[On('select-agent')]
    public function selectAgent($agentId)
    {
        $this->hasSelection = true;

        $this->ensureThread();
        $agent = Agent::find($agentId);
        if ($agent) {
            $this->thread->agent_id = $agent->id;
            $this->thread->model = 'command-r-plus';
            if ($agent->model) {
                $this->thread->model = $agent->model;
            }
            if (auth()->check() && auth()->user()->isPro()) {
                if ($agent->pro_model) {
                    $this->thread->model = $agent->pro_model;
                }
            }
            session()->put('lastModel', $this->thread->model);
            session()->put('lastAgent', $agent->id);
            $this->thread->save();
        }
    }

    #[On('select-model')]
    public function selectModel($model): void
    {
        $this->hasSelection = true;
        $this->ensureThread();
        $ac = $model ? Models::hasModelAccess($model) : false;
        if ($ac) {
            $this->thread->model = $model;
        } else {
            $defaultModel = Models::getDefaultModel();
            $this->thread->model = $defaultModel;
        }

        session()->put('lastModel', $this->thread->model);
        session()->forget('lastAgent');
        $this->thread->agent_id = null;
        $this->thread->save();
    }

    #[On('no-more-messages')]
    public function noMoreMessages(): void
    {
        // Redirect to homepage
        $this->showNoMoreMessages = true;
    }

    public function sendMessage(): void
    {
        if ($this->pending) {
            return;
        }

        $useModel = $this->thread->model;
        $useAgent = $this->thread->agent;
        if ($useAgent) {
            $maxCost = $useAgent->getPriceRange()['max'];
            if ($maxCost > 0) {
                if (! auth()->check()) {
                    $this->alert('error', 'You need to login to chat with this agent');

                    return;
                }
                if (auth()->user()->canBypassPayments()) {
                    $this->alert('info', 'Bypassed funds lock for '.$maxCost.' sats');
                } else {

                    $user = auth()->user();
                    if ($user->getAvailableSatsBalanceAttribute() < $maxCost) {
                        $this->alert('error', 'You need at least '.$maxCost.' sats to chat with this agent');

                        return;
                    }
                    try {
                        $lockId = $user->lockSats($maxCost);
                        $this->fundLocksIds[] = $lockId;
                        // Log::info('Locked funds for agent '.$useAgent->name.' for '.$maxCost.' sats');
                    } catch (Exception $e) {
                        $this->alert('error', 'Failed to lock funds');
                        Log::error($e);

                        return;
                    }
                }
            }

        }

        // Save this input even after we clear the form this variable is tied to
        $this->input = $this->message_input;
        $this->images_to_upload = $this->images;

        // Append the message to the chat
        $this->messages[] = [
            'body' => $this->input,
            'sender' => 'You',
            'user_id' => auth()->id(), // Add user_id if logged in
            'session_id' => auth()->check() ? null : Session::getId(), // Add session_id if not logged in
            'agent_id' => $useAgent->id ?? null,
            'model' => ! $useAgent ? $useModel : null,
            'input_tokens' => null,
            'output_tokens' => null,
        ];

        // Clear the input
        $this->message_input = '';
        $this->pending = true;
        $this->images = [];

        // Call simpleRun after the next render
        $this->dispatch('message-created');
        if (! $useAgent) {
            $this->js('$wire.simpleRun()');
        } else {
            $this->js('$wire.agentRun()');
        }
    }

    private function handleImageInput(): string
    {
        $imageDescriptions = '';
        if (! empty($this->images_to_upload)) {
            $imageService = new ImageService();
            foreach ($this->images_to_upload as $image) {
                $imageDescriptions .= $imageService->getImageDescription($image, $this->thread)."\n\n";
            }
            if ($imageDescriptions) {
                $imageDescriptions = str_replace('-->', ' ', $imageDescriptions);
                $imageDescriptions = "<!-- $imageDescriptions\n\nDo not mention or imply that you have not actually seen the image.-->\n\n";
            }
            $this->images_to_upload = [];
        }

        return $imageDescriptions;
    }

    private function getStreamingCallback(): callable
    {
        return function ($content, bool $replace = false) {
            $this->stream(
                to: 'streamtext'.$this->thread->id,
                content: $content,
                replace: $replace,
            );
        };
    }

    public function simpleRun(): void
    {
        // Convert any images to messages with descriptions generated by vision LLM
        $this->input = $this->handleImageInput().$this->input;

        // Authenticate user session or proceed without it
        $sessionId = auth()->check() ? null : Session::getId();

        // Save user message to the thread
        $userMessage = $this->thread->messages()->create([
            'body' => $this->input,
            'session_id' => $sessionId,
            'user_id' => auth()->id() ?? null,
            'input_tokens' => null,
            'output_tokens' => null,
        ]);

        $systemPrompt = auth()->user()->system_prompt ?? '';
        $inference = new SimpleInferencer();
        $output = $inference->inference($this->input, $this->thread->model, $this->thread, $this->getStreamingCallback(), $systemPrompt);

        // Append the response to the chat
        // Save the agent's response to the thread
        $message = $this->thread->messages()->create([
            'body' => $output['content'],
            'model' => $this->thread->model,
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
            'agent_id' => null,
            'input_tokens' => $output['input_tokens'],
            'output_tokens' => $output['output_tokens'],
        ])->toArray();
        $this->messages[] = $message;

        // Update the original user message with the input tokens
        $userMessage->update(['input_tokens' => $output['prompt_tokens']]);

        // Reset pending status and scroll to the latest message
        $this->pending = false;

        // Optionally notify other components of the new message
        $this->dispatch('message-created');

        if (isset($output['error'])) {
            $this->alert('error', $output['error']);
        }
    }

    public function agentRun(): void
    {
        $logger = new LocalLogger();
        $logger->log("Running agent with RAG. Input: {$this->input}");

        try {

            $sessionId = auth()->check() ? null : Session::getId();
            $uuid = $sessionId ? hash('sha256', $sessionId) : PoolUtils::uuid();

            // Save user message to the thread
            $this->thread->messages()->create([
                'body' => $this->input,
                'session_id' => $sessionId,
                'user_id' => auth()->id() ?? null,
                'agent_id' => $this->thread->agent->id,
            ]);

            $poolRag = new PoolRag(); // Generate history
            $query = $poolRag->history($this->thread)->summary();

            $agent = $this->thread->agent;
            if (! $agent) {
                $this->alert('error', 'Agent not found');

                return;
            }

            $documents = AgentFile::where('agent_id', $agent->id)->pluck('url')->toArray();
            $tools = $agent->externalTools()->pluck('external_uid')->toArray();

            if (count($documents) > 0 && $agent->is_rag_ready) {
                $this->alert('warning', 'Agent is still training..');
            }

            PoolUtils::sendRAGJob($agent->id, $this->thread->id, $uuid, $documents, $query, $tools);

        } catch (Exception $e) {
            $this->alert('error', 'An Error occurred, please try again later');
            Log::error($e);
        }
    }

    #[On('echo:threads.{thread.id},PoolJobReady')]
    public function processPoolJob($event): void
    {

        $logger = new OpenObserveLogger([

        ]);
        // Authenticate user session or proceed without it
        $sessionId = auth()->check() ? null : Session::getId();

        $job = PoolJob::where('thread_id', $this->thread->id)->find($event['job']['id']);
        $this->selectAgent($job['agent_id']);

        $agent = $job->agent;

        /////////////// PAYMENTS
        $payService = app(PaymentService::class);

        // Pay agent with user balance
        if ($agent->sats_per_message > 0) {
            if (auth()->check() && auth()->user()->canBypassPayments()) {
                $this->alert('info', 'Bypassed payment of '.$agent->sats_per_message.' sats');
            } else {
                try {
                    $paid = $payService->payAgentForMessage($agent->id, $agent->sats_per_message);
                    if (! $paid) {
                        throw new Exception('Failed to pay '.$agent->sats_per_message.' sats');
                    }
                } catch (Exception $e) {
                    $this->alert('error', $e->getMessage());

                    return;
                }
            }
        }

        // Track and pay tools with user balance
        $meta = json_decode($job->meta, true);
        $usedToolIds = $meta['usedTools'] ?? [];
        $usedTools = [];
        $availableTools = PoolUtils::getTools();
        foreach ($usedToolIds as $toolId) {
            $tool = null;
            foreach ($availableTools as $availableTool) {
                $logger->log('info', 'Checking tool '.json_encode($availableTool).$toolId);
                if (isset($availableTool['id']) && $availableTool['id'] == $toolId) {
                    $logger->log('info', 'Found tool '.$toolId);
                    $tool = $availableTool;
                    break;
                }
            }
            if (isset($tool)) {
                $logger->log('info', 'Used tool '.$tool['meta']['name']);
                $usedTools[] = $tool;
            }
        }

        // Pay and compute total tool cost
        $totalToolsCost = 0;
        foreach ($usedTools as $usedTool) {
            try {
                $meta = $usedTool['meta'];

                $sats = PoolUtils::getToolPriceInSats($usedTool);
                if ($sats == 0) {
                    continue;
                }

                $lnAddress = $meta['payment'];
                $id = $meta['id'];

                $plugin = null;

                // check if it's a platform plugin
                if (strpos($id, 'oaplugin') === 0) {
                    $id = substr($id, 8);
                    $plugin = Plugin::find($id);
                }

                if ($plugin) { // If plugin pay with internal payment system
                    if (auth()->check() && auth()->user()->canBypassPayments()) {
                        $this->alert('info', 'Bypassed payment of '.$sats.' sats to plugin '.$plugin->name);
                    } else {
                        $payService->payPluginForMessage($plugin->id, $sats);
                    }
                } else { // If external tool pay with lightning
                    if (isset($lnAddress) && $lnAddress && strpos($lnAddress, 'lightning:') === 0) {
                        if (auth()->check() && auth()->user()->canBypassPayments()) {
                            $this->alert('info', 'Bypassed payment of '.$sats.' sats to untracked tool '.$usedTool);
                        } else {
                            $lnAddress = substr($lnAddress, 10);
                            // TODO (?)
                            $logger->log('info', 'Currently unsupported: Requested payment for untracked tool '.$usedTool.' to '.$lnAddress.' for '.$sats.' sats');
                            $totalToolsCost += $sats;
                        }
                    } else {
                        $logger->log('info', 'Unsupported payment address for tool '.$usedTool.': '.$lnAddress);
                    }
                }
            } catch (Exception $e) {
                $this->alert('error', $e->getMessage());
                Log::error($e);
            }
        }

        // Track total cost for average stats
        $agent->trackToolsCost($totalToolsCost + $agent->sats_per_message);

        // unlock funds
        foreach ($this->fundLocksIds as $lockId) {
            // Log::info('Unlocking funds for lock '.$lockId);
            auth()->user()->unlockSats($lockId);
        }
        $this->fundLocksIds = [];

        // Simply do it
        $inferencer = new PoolInference();
        $output = $inferencer->inference($this->thread->model, $job, $this->getStreamingCallback());

        // Append the response to the chat
        // Save the agent's response to the thread
        $message = $this->thread->messages()->create([
            'body' => $output['content'],
            'model' => $this->thread->model,
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
            'agent_id' => $this->thread->agent->id,
            'input_tokens' => $output['input_tokens'],
            'output_tokens' => $output['output_tokens'],
        ])->load('agent')->toArray();

        $this->messages[] = $message;

        // Reset pending status and scroll to the latest message
        $this->pending = false;

        // Optionally notify other components of the new message
        $this->dispatch('message-created');

        if (isset($output['error'])) {
            $this->alert('error', $output['error']);
        }
    }

    // #[On('echo:rags.{rag.id},AgentRagReady')]
    // public function process_agent_rag($event){
    //     $agent = Agent::find($event['agent_id']);
    //     if($agent){
    //         if($this->selectedAgent == $agent->id && $agent->is_rag_ready){
    //             $this->pending = false;
    //         }
    //     }
    // }

    public function render()
    {
        return view('livewire.chat');
    }
}
