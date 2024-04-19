<?php

namespace App\Livewire;

use App\AI\Models;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class Settings extends Component
{
    public $formattedDefaultModel;

    public $models = Models::MODELS;

    public function mount()
    {
        $this->formattedDefaultModel = $this->getFormattedModelName(auth()->user()->default_model);
    }

    private function getFormattedModelName($modelKey)
    {
        return $this->models[$modelKey]['name'] ?? 'Unknown Model';
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
        if (Auth::check() && Auth::user()->isPro()) {
            return 'pro';
        } elseif (Auth::check()) {
            return 'user';
        } else {
            return 'guest';
        }
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
