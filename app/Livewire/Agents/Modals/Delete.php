<?php

namespace App\Livewire\Agents\Modals;

use App\Livewire\Agents\Index;
use App\Models\Agent;
use Illuminate\Support\Facades\Storage;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Delete extends ModalComponent
{
    use LivewireAlert;

    public $agent;

    public $name;

    public $agent_id;

    public function mount(Agent $agent)
    {
        $this->name = $agent->name;
        $this->agent_id = $agent->id;
    }

    public function delete()
    {

        $agent = Agent::with('documents')->find($this->agent_id);

        if ($agent->user_id !== auth()->user()->id) {

            $this->alert('error', 'Permission Denied..');
            $this->closeModal();

        } else {
            // Delete documents from storage and database
            $agent->documents->each(function ($document) {
                Storage::disk($document->disk)->delete($document->path);
                $document->delete();
            });

            // Now delete the agent
            $agent->delete();

            $this->dispatch('agent_deleted')->to(Index::class);

            $this->closeModal();

            $this->alert('success', 'Agent deleted successfully');

        }

    }

    public function render()
    {
        return view('livewire.agents.modals.delete');
    }
}
