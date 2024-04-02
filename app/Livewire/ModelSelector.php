<?php

namespace App\Livewire;

use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = 'mixtral-8x7b-32768'; // Default selection

    public function selectModel($model)
    {
        $this->selectedModel = $model;
        $this->dispatch('select-model', $model);
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
