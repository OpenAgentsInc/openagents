<?php

namespace App\Livewire;

use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;

class LightningAddressDisplay extends Component
{
    use LivewireAlert;

    protected $listeners = ['copiedToClipboard' => 'copiedToClipboard'];

    public $lightningAddress;

    public function mount($lightningAddress = null)
    {
        $this->lightningAddress = $lightningAddress;
    }

    public function render()
    {
        return view('livewire.lightning-address-display');
    }

    public function copiedToClipboard()
    {
        $this->alert('success', 'Copied to clipboard');

    }
}
