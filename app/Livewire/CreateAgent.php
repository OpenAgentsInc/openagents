<?php

namespace App\Livewire;

use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;
use Livewire\WithFileUploads;

class CreateAgent extends Component
{
    use LivewireAlert, WithFileUploads;

    public $name;

    public $description;

    public $instructions;

    public $files = [];

    public function render()
    {
        return view('livewire.create-agent');
    }
}
