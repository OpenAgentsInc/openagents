<?php

namespace App\Livewire;

use App\AI\Models;
use Exception;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\On;
use Livewire\Component;

class Settings extends Component
{
    use LivewireAlert;

    public $selectedModel;

    public $autoscroll;

    public $lightning_address;

    public $system_prompt;

    public $models = Models::MODELS;

    public function mount()
    {
        if (! auth()->check()) {
            return redirect('/');
        }

        $this->lightning_address = auth()->user()->lightning_address;

        $this->selectedModel = auth()->user()->default_model ?? Models::getDefaultModel();
        $this->system_prompt = auth()->user()->system_prompt ?? '';
        $this->autoscroll = auth()->user()->autoscroll; // Initialize autoscroll

    }

    public function updateSystemPrompt()
    {
        try {
            $this->validate([
                'system_prompt' => 'nullable|string',
            ]);
        } catch (Exception $e) {
            $this->alert('error', 'Invalid system prompt');

            return;
        }

        auth()->user()->update(['system_prompt' => trim($this->system_prompt)]);

        $this->alert('success', 'Updated system prompt');
    }

    public function updateLightningAddress()
    {
        try {
            $this->validate([
                'lightning_address' => 'nullable|string|email',
            ]);
        } catch (Exception $e) {
            $this->alert('error', 'Invalid lightning address');

            return;
        }

        auth()->user()->update(['lightning_address' => $this->lightning_address]);

        $this->alert('success', 'Updated Lightning address');
    }

    #[On('select-model')]
    public function selectModel($modelKey)
    {
        auth()->user()->update(['default_model' => $modelKey]);
        $this->selectedModel = $modelKey;
        $this->alert('success', 'Updated default model');
    }

    public function toggleAutoscroll() // Add this method
    {
        $this->autoscroll = ! $this->autoscroll;
        auth()->user()->update(['autoscroll' => $this->autoscroll]);
        $this->alert('success', 'Updated autoscroll');
    }

    public function render()
    {
        return view('livewire.settings');
    }
}
