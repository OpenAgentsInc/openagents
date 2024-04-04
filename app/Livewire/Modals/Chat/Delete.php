<?php

namespace App\Livewire\Modals\Chat;

use LivewireUI\Modal\ModalComponent;

class Delete extends ModalComponent
{
    public function delete()
    {

        //save data and close modal

        $this->closeModal();
    }

    public function render()
    {
        return view('livewire.modals.chat.delete');
    }
}
