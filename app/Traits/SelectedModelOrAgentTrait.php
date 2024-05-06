<?php

namespace App\Traits;

use App\AI\Models;
use App\Models\Thread;

trait SelectedModelOrAgentTrait
{
    public $selectedModel = '';

    public $selectedAgent = [];

    public function setSelectedModel($model)
    {
        $this->selectedModel = $model;
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
}
