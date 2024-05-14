<?php

namespace App\Livewire;

use App\Models\PrismSinglePayment;
use Livewire\Component;

class Explorer extends Component
{
    public $prismSinglePayments = [];

    public function mount()
    {
        $this->prismSinglePayments = PrismSinglePayment::all();
    }

    public function render()
    {
        return view('livewire.explorer');
    }
}
