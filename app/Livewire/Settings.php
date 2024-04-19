<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class Settings extends Component
{
    public $formattedDefaultModel;

    public $selectedModel;

    public $models = Models::MODELS;

    public function mount()
    {
        $this->selectedModel = auth()->user()->default_model;
    }

    public function setDefaultModel($modelKey)
    {
        auth()->user()->update(['default_model' => $modelKey]);
        $this->formattedDefaultModel = $this->getFormattedModelName($modelKey);
    }

    public function render()
    {
        return view('livewire.settings');
    }

    protected function getUserAccess()
    {
        return Models::getUserAccess();
    }

    protected function getModelIndicator($model, $userAccess)
    {
        return Models::getModelIndicator($model, $userAccess);
    }
}
