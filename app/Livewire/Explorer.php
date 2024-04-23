<?php

namespace App\Livewire;

use App\Models\PrismMultiPayment;
use App\Models\PrismSinglePayment;
use Livewire\Component;

class Explorer extends Component
{
    public $prismMultiPayments = [];

    public $prismSinglePayments = [];

    public function mount()
    {
        $this->prismMultiPayments = PrismMultiPayment::all();
        $this->prismSinglePayments = PrismSinglePayment::all();
    }

    public function render()
    {
        return view('livewire.explorer')->layout('components.layouts.store');
    }
}
