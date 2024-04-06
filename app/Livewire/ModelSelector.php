<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = '';

    public $formattedModel = '';

    public $models = Models::MODELS;

    public function mount()
    {
        $this->selectedModel = Models::getDefaultModel();
        $this->formattedModel = Models::getModelName($this->selectedModel);
    }

    public function selectModel($model)
    {
        $this->selectedModel = $model;
        $this->formattedModel = Models::getModelName($model);
        $this->dispatch('select-model', $model);
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
