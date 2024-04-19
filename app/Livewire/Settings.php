<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Attributes\On;
use Livewire\Component;

class Settings extends Component
{
    public $selectedModel;

    public $models = Models::MODELS;

    public function mount()
    {
        // If not logged in, redirect to /
        if (! auth()->check()) {
            return redirect('/');
        }

        $this->selectedModel = auth()->user()->default_model ?? Models::getDefaultModel();
    }

    #[On('select-model')]
    public function selectModel($modelKey)
    {
        auth()->user()->update(['default_model' => $modelKey]);
        $this->selectedModel = $modelKey;
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
