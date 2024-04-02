<?php

namespace App\Livewire\Modals\Chat;

use LivewireUI\Modal\ModalComponent;

class Rename extends ModalComponent
{
    public function update()
    {

        //save data and close modal

        $this->closeModal();
    }

    public function render()
    {
        return view('livewire.modals.chat.rename');
    }
}
