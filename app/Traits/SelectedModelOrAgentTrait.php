<?php

namespace App\Traits;

use App\AI\Models;
use App\Models\Agent;
use App\Models\Thread;
use Livewire\Attributes\On;

trait SelectedModelOrAgentTrait
{
    public string $selectedModel = '';

    public array $selectedAgent = [];

    // Track if a selection has been made by user
    public bool $hasSelection = false;

    #[On('select-model')]
    public function selectedModel($model): void
    {
        $this->selectedModel = $model;
        $this->selectedAgent = [];
        $this->hasSelection = true;
    }

    #[On('select-agent')]
    public function selectedAgent($id): void
    {
        $this->selectedAgent = $this->getSelectedAgentFromId($id);
        $this->hasSelection = true;
    }

    private function getSelectedAgentFromId($id): array
    {
        $agent = Agent::find($id);

        return [
            'id' => $agent->id,
            'name' => $agent->name,
            'description' => $agent->about,
            'instructions' => $agent->prompt,
            'image' => $agent->image_url,
            'is_rag_ready' =>  $agent->is_rag_ready,
            'created_at' => $agent->created_at,
            'capabilities' => $this->safeDecode($agent->capabilities),
        ];
    }

    private function safeDecode($json)
    {
        return is_string($json) ? json_decode($json, true) : [];
    }

    public function setModelOrAgentForThread(Thread $thread): void
    {
        $messages = $this->thread->messages()
            ->with('agent:image,id,name,about,prompt,is_rag_ready,created_at')
            ->orderBy('created_at', 'asc')
            ->get()
            ->toArray();

        // If the thread has a last message with an agent or otherwise has an agent, set the selected agent
        $lastMessage = end($messages);
        if (! empty($lastMessage['agent_id'])) {
            $this->selectedAgent = $this->getSelectedAgentFromMessage($lastMessage);
        } elseif (! empty($this->thread->agent_id)) {
            $this->selectedAgent = $this->getSelectedAgentFromThread();
        } elseif (session()->has('selectedAgent')) {
            $this->selectedAgent = $this->getSelectedAgentFromId(session()->get('selectedAgent'));
            session()->put('agent', $this->selectedAgent['id']);
            session()->forget('selectedAgent');
        } elseif (session()->has('agent')) {
            $this->selectedAgent = $this->getSelectedAgentFromSession();
            session()->forget('agent');
        } else {
            $this->selectedModel = Models::getModelForThread($this->thread);
        }

        // If the agent has codebase capability, fire an event to notify the sidebar component of the active agent & selected codebases
        if ($this->selectedAgent && optional($this->selectedAgent['capabilities'])['codebase_search']) {
            $this->dispatch('codebase-agent-selected', $this->selectedAgent['id']);
        }
    }

    private function getSelectedAgentFromMessage($message): array
    {
        return [
            'id' => $message['agent_id'],
            'name' => $message['agent']['name'],
            'description' => $message['agent']['about'],
            'instructions' => $message['agent']['prompt'],
            'image' => $message['agent']['image_url'],
             'is_rag_ready' =>  $message['agent']['is_rag_ready'],
             'created_at' => $message['agent']['created_at'],
            'capabilities' => $this->safeDecode($message['agent']['capabilities'] ?? null),
        ];
    }

    private function getSelectedAgentFromThread(): array
    {
        return [
            'id' => $this->thread->agent_id,
            'name' => $this->thread->agent->name,
            'description' => $this->thread->agent->about,
            'instructions' => $this->thread->agent->prompt,
            'image' => $this->thread->agent->image_url,
            'is_rag_ready' =>  $this->thread->agent->is_rag_ready,
            'created_at' =>  $this->thread->agent->created_at,
            'capabilities' => $this->safeDecode($this->thread->agent->capabilities),
        ];
    }

    private function getSelectedAgentFromSession(): array
    {
        $agentId = session('agent');
        $agent = Agent::find($agentId);

        return [
            'id' => $agent->id,
            'name' => $agent->name,
            'description' => $agent->about,
            'instructions' => $agent->prompt,
            'image' => $agent->image_url,
            'is_rag_ready' =>  $agent->is_rag_ready,
            'created_at' =>  $agent->created_at,
            'capabilities' => $this->safeDecode($agent->capabilities),
        ];
    }
}
