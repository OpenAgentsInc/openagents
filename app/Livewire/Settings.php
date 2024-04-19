<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Attributes\On;
use Livewire\Component;

class Settings extends Component
{
    public $selectedModel;

    public $autoscroll; // Add this line

    public $models = Models::MODELS;

    public function mount()
    {
        if (! auth()->check()) {
            return redirect('/');
        }

        $this->selectedModel = auth()->user()->default_model ?? Models::getDefaultModel();
        $this->autoscroll = auth()->user()->autoscroll; // Initialize autoscroll
    }

    #[On('select-model')]
    public function selectModel($modelKey)
    {
        auth()->user()->update(['default_model' => $modelKey]);
        $this->selectedModel = $modelKey;
    }

    public function toggleAutoscroll() // Add this method
    {
        $this->autoscroll = ! $this->autoscroll;
        auth()->user()->update(['autoscroll' => $this->autoscroll]);
    }

    public function render()
    {
        return view('livewire.settings');
    }
}
