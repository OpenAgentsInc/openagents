<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class ModelDropdown extends Component
{
    public $selectedModel;

    public $formattedModel = '';

    public $models;

    public $action;

    public $isOpen = false;

    public function mount($selectedModel, $models, $action)
    {
        $this->selectedModel = $selectedModel;
        $this->formattedModel = Models::getModelName($this->selectedModel);
        $this->models = $models;
        $this->action = $action;
    }

    public function selectModel($model)
    {
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

    public function getUserAccess()
    {
        return Models::getUserAccess();
    }

    public function hasModelAccess($model, $userAccess)
    {
        return Models::hasModelAccess($model, $userAccess);
    }

    protected function isProModelSelected($model)
    {
        return Models::isProModelSelected($model);
    }

    public function toggleDropdown()
    {
        $this->isOpen = ! $this->isOpen;
    }

    public function render()
    {
        return view('livewire.model-dropdown');
    }

    public function getModelIndicator($modelKey)
    {
        return Models::getModelIndicator($modelKey, $this->getUserAccess());
    }
}
