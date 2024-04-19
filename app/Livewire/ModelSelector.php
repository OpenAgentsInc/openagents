<?php

namespace App\Livewire;

use App\AI\Models;
use Illuminate\Support\Facades\Auth;
use Livewire\Attributes\On;
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

    #[On('model-selected')]
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

    protected function getUserAccess()
    {
        if (Auth::check() && Auth::user()->isPro()) {
            return 'pro';
        } elseif (Auth::check()) {
            return 'user';
        } else {
            return 'guest';
        }
    }

    protected function hasModelAccess($model, $userAccess)
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $requiredAccess = $modelDetails['access'];
            $accessLevels = ['guest', 'user', 'pro'];
            $userAccessIndex = array_search($userAccess, $accessLevels);
            $requiredAccessIndex = array_search($requiredAccess, $accessLevels);

            return $userAccessIndex >= $requiredAccessIndex;
        }

        return false;
    }

    protected function isProModelSelected($model)
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            return $modelDetails['access'] === 'pro';
        }

        return false;
    }

    public function render()
    {
        return view('livewire.model-selector');
    }

    protected function getModelIndicator($model, $userAccess)
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $requiredAccess = $modelDetails['access'];
            $accessLevels = ['guest', 'user', 'pro'];
            $userAccessIndex = array_search($userAccess, $accessLevels);
            $requiredAccessIndex = array_search($requiredAccess, $accessLevels);

            if ($userAccessIndex < $requiredAccessIndex) {
                if ($requiredAccess === 'pro') {
                    return 'Pro';
                } else {
                    return 'Join';
                }
            }
        }

        return '';
    }
}
