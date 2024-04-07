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

        $this->thread->update(['title' => $this->title]);

        $this->alert('success', 'Updated successfully');
        $this->dispatch('thread-update', $this->thread->id);

        $this->closeModal();
    }

    public function render()
    {
        return view('livewire.modals.chat.rename');
    }
}
