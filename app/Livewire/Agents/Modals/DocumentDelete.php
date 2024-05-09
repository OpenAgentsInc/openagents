<?php

namespace App\Livewire\Agents\Modals;

use Livewire\Component;
use App\Models\AgentFile;
use LivewireUI\Modal\ModalComponent;
use Illuminate\Support\Facades\Storage;
use App\Livewire\Agents\Partials\Documents;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class DocumentDelete extends ModalComponent
{
    use LivewireAlert;

    public $document;

    public $name;

    public $document_id;

    public function mount(AgentFile $document)
    {
        $this->name = $document->name;
        $this->document_id = $document->id;
    }

    public function delete()
    {

        $document = AgentFile::with('agent')->find($this->document_id);

        if ($document->agent->user_id !== auth()->user()->id) {

            $this->alert('error', 'Permission Denied..');
            $this->closeModal();
        } else {
            // Delete documents from storage and database
            Storage::disk($document->disk)->delete($document->path);

            // Now delete the agent
            $document->delete();

            $this->dispatch('document_deleted')->to(Documents::class);

            $this->closeModal();

            $this->alert('success', 'Document deleted successfully');
        }
    }

    public function render()
    {
        return view('livewire.agents.modals.document-delete');
    }
}
