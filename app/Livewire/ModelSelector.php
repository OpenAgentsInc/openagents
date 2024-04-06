<?php

namespace App\Livewire;

use Livewire\Component;

class ModelSelector extends Component
{
    public $selectedModel = 'mistral-large-latest'; // Default selection

    public $formattedModel = 'Mistral Large';

    public $modelnames = [
        'mistral-tiny' => 'Mistral Tiny',
        'mistral-small-latest' => 'Mistral Small',
        'mistral-medium-latest' => 'Mistral Medium',
        'mistral-large-latest' => 'Mistral Large',
        'open-mixtral-8x7b' => 'Open Mixtral 8x7B',
        'open-mistral-7b' => 'Open Mistral 7B',
        //        'mixtral-8x7b-32768' => 'Mixtral (Groq)',
        //        'gpt-4' => 'GPT-4',
        //        'claude' => 'Claude',
        //        'gemini' => 'Gemini',
    ];

    public function selectModel($model)
    {
        $this->selectedModel = $model;
        $this->formattedModel = $this->getModelName();
        $this->dispatch('select-model', $model);
    }

    public function getModelName()
    {
        return $this->modelnames[$this->selectedModel] ?? 'Unknown Model';
    }

    public function render()
    {
        return view('livewire.model-selector');
    }
}
