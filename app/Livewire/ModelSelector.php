<?php

namespace App\Livewire;

use App\AI\Models;
use Illuminate\Support\Facades\Auth;
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
        $userAccess = $this->getUserAccess();

        // Check if the selected model requires "pro" access and the user is not a "pro" user
        if ($this->isProModelSelected($model) && $userAccess !== 'pro') {
            $this->dispatch('openModal', 'modals.upgrade');

            return;
        }

        // Check if the user has access to the selected model
        if ($this->hasModelAccess($model, $userAccess)) {
            $this->selectedModel = $model;
            $this->formattedModel = Models::getModelName($model);
            $this->dispatch('select-model', $model);
        } else {
            dd('no access to that model!');
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

    protected function isProModelSelected($model)
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            return $modelDetails['access'] === 'pro';
        }

        return false;
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
                    return 'Sign up';
                }
            }
        }

        return '';
    }
}
