<?php

namespace App\Livewire\Modals\Chat;

use App\Models\Thread;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Delete extends ModalComponent
{
    use LivewireAlert;

    public $thread;

    public $title;

    public $thread_id;

    public function mount(Thread $thread)
    {
        $this->title = $thread->title;
        $this->thread_id = $thread->id;
    }

    public function delete()
    {

        $thread = Thread::find($this->thread_id);

        if ($thread && ! empty($thread)) {

            // Delete related messages
            $thread->messages()->delete();

            // Now delete the thread
            $thread->delete();

            $this->alert('success', 'Thread Deleted');

            $this->dispatch('thread-update');

            $this->closeModal();
        } else {
            $this->alert('error', 'An unknown Error occured');
            $this->closeModal();
        }
    }

    public function render()
    {
        return view('livewire.modals.chat.delete');
    }
}
