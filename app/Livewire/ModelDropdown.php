<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class ModelDropdown extends Component
{
    public $selectedAgent;

    public $selectedModel;

    public $formattedModelOrAgent = '';

    public $models;

    public $action;

    public $isOpen = false;

    public $picture;

    public $showAgents = false;

    public function mount($selectedAgent, $selectedModel, $models, $action, $showAgents = false)
    {
        $this->selectedAgent = $selectedAgent;
        $this->selectedModel = $selectedModel;

        // If selectedAgent is an array with >=3 elements, it means the user has selected an agent
        if (is_array($selectedAgent) && count($selectedAgent) >= 3) {
            $this->formattedModelOrAgent = $this->selectedAgent['name'];
            //            dd($this->selectedAgent);
            $this->picture = $this->selectedAgent['image'];
        } else {
            $this->formattedModelOrAgent = Models::getModelName($this->selectedModel);
            $this->picture = Models::getModelPicture($this->selectedModel);
        }

        // Filter out 'hidden' models - if access is 'hidden'
        $this->models = collect($models)->filter(function ($model) {
            return $model['access'] !== 'hidden';
        })->toArray();

        $this->action = $action;
        $this->showAgents = $showAgents;
    }

    public function selectModel($model)
    {
        $this->selectedAgent = '';
        session()->forget('selectedAgent');

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
            $this->formattedModelOrAgent = Models::getModelName($model);
            $this->dispatch('select-model', $model);
        }

        $this->picture = Models::getModelPicture($this->selectedModel);
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
