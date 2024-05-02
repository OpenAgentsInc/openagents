<?php

namespace App\Livewire\Auth;

use Jantinnerezo\LivewireAlert\LivewireAlert;
use App\Vendors\WireElements\Modal\ModalComponent;


class Join extends ModalComponent
{
    use LivewireAlert;

    public function render()
    {
        return view('livewire.auth.join');
    }


        /**
     * Supported: 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl', '6xl', '7xl'
     */
    public static function modalMaxWidth(): string
    {
        return 'auth';
    }



}
