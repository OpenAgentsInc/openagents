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

        $this->selectedModel = Models::getModelForThread($this->thread);

        if (session()->has('selectedModel')) {
            session()->forget('selectedModel');
        }

        if (session()->has('selectedAgent')) {
            $agent = $this->agents->find(session('selectedAgent'));

            $this->selectedAgent = [
                'id' => $agent->id,
                'title' => $agent->name,
                'description' => $agent->about,
                'image' => $agent->image_url,
            ];

            session()->forget('selectedAgent');
        }
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
