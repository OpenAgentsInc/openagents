<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class Store extends Component
{
    public $models = Models::MODELS;

    public function render()
    {
        return view('livewire.store')
            ->layout('components.layouts.store');
    }
}
