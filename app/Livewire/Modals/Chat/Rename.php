<?php

namespace App\Livewire\Modals\Chat;

use App\Models\Thread;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Rename extends ModalComponent
{
    use LivewireAlert;

    public Thread $thread;

    public $title;

    public function mount(Thread $thread)
    {
        $this->title = $thread->title;
    }

    public function update()
    {

        $this->validate([
            'title' => 'required',
        ]);

        $this->thread->title = $this->title;
        $this->thread->save();

        $this->alert('success', 'Name Updated');

        $this->dispatch('thread-update');

        $this->closeModal();
    }

    public function render()
    {
        return view('livewire.modals.chat.rename');
    }
}
