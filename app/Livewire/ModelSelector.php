<?php

namespace App\Livewire;

use App\AI\Models;
use App\Traits\SelectedModelOrAgentTrait;
use Livewire\Component;

class ModelSelector extends Component
{
    use SelectedModelOrAgentTrait;

    public $models;

    public $agents;

    public $thread;

    public function mount()
    {
        $this->models = Models::MODELS;

        if (! $this->thread) {
            return;
        }

        $this->setModelOrAgentForThread($this->thread);
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
