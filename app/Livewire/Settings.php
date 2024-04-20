<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Attributes\On;
use Livewire\Component;

class Settings extends Component
{
    public $selectedModel;

    public $autoscroll;

    public $lightning_address;

    public $models = Models::MODELS;

    public function mount()
    {
        if (! auth()->check()) {
            return redirect('/');
        }

        $this->lightning_address = auth()->user()->lightning_address;

        $this->selectedModel = auth()->user()->default_model ?? Models::getDefaultModel();
        $this->autoscroll = auth()->user()->autoscroll; // Initialize autoscroll
    }

    public function updateLightningAddress()
    {
        $this->validate([
            'lightning_address' => 'nullable|string',
        ]);

        auth()->user()->update(['lightning_address' => $this->lightning_address]);
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
