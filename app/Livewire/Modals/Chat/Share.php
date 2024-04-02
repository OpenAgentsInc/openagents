<?php

namespace App\Livewire\Modals\Chat;

use Livewire\Component;
use LivewireUI\Modal\ModalComponent;

class Share extends ModalComponent
{
    public function render()
    {
        return view('livewire.modals.chat.share');
    }

    /**
     * Supported: 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl', '6xl', '7xl'
     */
    public static function modalMaxWidth(): string
    {
        return '3xl';
    }
}
