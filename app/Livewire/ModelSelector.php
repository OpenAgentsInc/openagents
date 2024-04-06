<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = 'mistral-small-latest'; // Default selection

    public $formattedModel = 'Mistral Small';

    public $models = Models::MODELS;

    public function selectModel($model)
    {
        $this->selectedModel = $model;
        $this->formattedModel = $this->getModelName();
        $this->dispatch('select-model', $model);
    }

    public function getModelName()
    {
        return $this->models[$this->selectedModel] ?? 'Unknown Model';
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
