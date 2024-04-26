<?php

namespace App\Livewire;

use Livewire\Component;

class CreateAgent extends Component
{
    public $description;

    public function render()
    {
        return view('livewire.create-agent');
    }
}
