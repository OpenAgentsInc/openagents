<?php

namespace App\Livewire;

use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = 'mixtral-8x7b-32768'; // Default selection

    public $formattedModel = 'Mixtral (Groq)';

    public $modelnames = [
        'mistral-large-latest' => 'Mistral Large',
        'mixtral-8x7b-32768' => 'Mixtral (Groq)',
        'gpt-4' => 'GPT-4',
        'claude' => 'Claude',
        'gemini' => 'Gemini',
    ];

    public function selectModel($model)
    {
        $this->selectedModel = $model;
        $this->formattedModel = $this->getModelName();
        $this->dispatch('select-model', $model);
    }

    public function getModelName()
    {
        $models = [
            'mistral-large-latest' => 'Mistral Large',
            'mixtral-8x7b-32768' => 'Mixtral (Groq)',
            'gpt-4' => 'GPT-4',
            'claude' => 'Claude',
            'gemini' => 'Gemini',
        ];

        return $models[$this->selectedModel] ?? 'Unknown Model';
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
