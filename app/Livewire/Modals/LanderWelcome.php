<?php

namespace App\Livewire\Modals;

use LivewireUI\Modal\ModalComponent;

class LanderWelcome extends ModalComponent
{
    /**
     * Supported: 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl', '6xl', '7xl'
     */
    public static function modalMaxWidth(): string
    {
        return '3xl';
    }

    public function render()
    {
        return view('livewire.modals.lander-welcome');
    }
}
