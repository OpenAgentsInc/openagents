<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Attributes\On;
use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = '';

    public $formattedModel = '';

    public $models;

    public $thread;

    public function mount()
    {
        $this->models = Models::MODELS;

        $this->selectedModel = Models::getModelForThread($this->thread);

        $this->formattedModel = Models::getModelName($this->selectedModel);
    }

    #[On('model-selected')]
    public function selectModel($model)
    {
        dd("selected model: $model");
        $userAccess = $this->getUserAccess();

        // If the user is not logged in, show the login modal for any model they don't have access to
        if ($userAccess === 'guest' && ! $this->hasModelAccess($model, $userAccess)) {
            $this->dispatch('openModal', 'auth.join');

            return;
        }

        // Check if the selected model requires "pro" access
        if ($this->isProModelSelected($model)) {
            // If the user is logged in but not a "pro" user, show the upgrade modal
            if ($userAccess !== 'pro') {
                $this->dispatch('openModal', 'modals.upgrade');

                return;
            }
        }

        // If the user has access to the selected model, update the selected model
        if ($this->hasModelAccess($model, $userAccess)) {
            $this->selectedModel = $model;
            $this->formattedModel = Models::getModelName($model);
            $this->dispatch('select-model', $model);
        }
    }

    protected function getUserAccess()
    {
        return Models::getUserAccess();
    }

    protected function hasModelAccess($model, $userAccess)
    {
        return Models::hasModelAccess($model, $userAccess);
    }

    protected function isProModelSelected($model)
    {
        return Models::isProModelSelected($model);
    }

    public function render()
    {
        return view('livewire.model-selector');
    }

    protected function getModelIndicator($model, $userAccess)
    {
        return Models::getModelIndicator($model, $userAccess);
    }
}
