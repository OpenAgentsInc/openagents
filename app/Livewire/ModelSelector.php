<?php

namespace App\Livewire;

use App\AI\Agents;
use App\AI\Models;
use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = '';

    public $selectedAgent = '';

    public $models;

    public $agents;

    public $thread;

    public function mount()
    {
        $this->models = Models::MODELS;
        $this->agents = Agents::AGENTS();

        if (! $this->thread) {
            return;
        }

        $messages = $this->thread->messages()
            ->with(['agent' => function ($query) {
                $query->select('id', 'name', 'about', 'prompt', 'image_url');
            }])
            ->orderBy('created_at', 'asc')
            ->get()
            ->toArray();
        // If the thread has a last message with an agent or otherwise has an agent, set the selected agent
        $lastMessage = end($messages);
        if (! empty($lastMessage['agent_id'])) {
            $this->selectedAgent = [
                'id' => $lastMessage['agent_id'],
                'name' => $lastMessage['agent']['name'],
                'description' => $lastMessage['agent']['about'],
                'instructions' => $lastMessage['agent']['prompt'],
                'image' => $lastMessage['agent']['image_url'],
            ];
        } elseif (! empty($this->thread->agent_id)) {
            $this->selectedAgent = [
                'id' => $this->thread->agent_id,
                'name' => $this->thread->agent->name,
                'description' => $this->thread->agent->about,
                'instructions' => $this->thread->agent->prompt,
                'image' => $this->thread->agent->image_url,
            ];
        } elseif (session()->has('agent')) {
            // If the selectedAgent session var is set, use it
            $this->selectedAgent = session('selectedAgent');
        } else {
            // Set the selected model
            $this->selectedModel = Models::getModelForThread($this->thread);
        }
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
