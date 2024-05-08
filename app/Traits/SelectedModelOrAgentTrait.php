<?php

namespace App\Traits;

use App\AI\Models;
use App\Models\Agent;
use App\Models\Thread;
use Livewire\Attributes\On;

trait SelectedModelOrAgentTrait
{
    public $selectedModel = '';

    public $selectedAgent = [];

    #[On('select-model')]
    public function selectedModel($model)
    {
        $this->selectedModel = $model;
        $this->selectedAgent = [];
    }

    #[On('select-agent')]
    public function selectedAgent($id)
    {
        $this->selectedAgent = $this->getSelectedAgentFromId($id);
    }

    private function getSelectedAgentFromId($id)
    {
        $agent = Agent::find($id);

        return [
            'id' => $agent->id,
            'name' => $agent->name,
            'description' => $agent->about,
            'instructions' => $agent->prompt,
            'image' => $agent->image_url,
            'capabilities' => json_decode($agent->capabilities, true),
        ];
    }

    public function setModelOrAgentForThread(Thread $thread)
    {
        $messages = $this->thread->messages()
            ->with('agent:image,id,name,about,prompt')
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

    private function getSelectedAgentFromMessage($message)
    {
        return [
            'id' => $message['agent_id'],
            'name' => $message['agent']['name'],
            'description' => $message['agent']['about'],
            'instructions' => $message['agent']['prompt'],
            'image' => $message['agent']['image_url'],
            'capabilities' => json_decode($message['agent']['capabilities'] ?? '[]', true),
        ];
    }

    private function getSelectedAgentFromThread()
    {
        $capabilities = [];
        if (is_string($this->thread->agent->capabilities)) {
            $capabilities = json_decode($this->thread->agent->capabilities, true);
        }

        return [
            'id' => $this->thread->agent_id,
            'name' => $this->thread->agent->name,
            'description' => $this->thread->agent->about,
            'instructions' => $this->thread->agent->prompt,
            'image' => $this->thread->agent->image_url,
            'capabilities' => $capabilities,
        ];
    }

    private function getSelectedAgentFromSession()
    {
        $agentId = session('agent');
        $agent = Agent::find($agentId);
        $capabilities = [];
        if (is_string($agent->capabilities)) {
            $capabilities = json_decode($agent->capabilities, true);
        }

        return [
            'id' => $agent->id,
            'name' => $agent->name,
            'description' => $agent->about,
            'instructions' => $agent->prompt,
            'image' => $agent->image_url,
            'capabilities' => $capabilities,
        ];
    }
}
