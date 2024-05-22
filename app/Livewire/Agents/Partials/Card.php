<?php

namespace App\Livewire\Agents\Partials;

use App\Models\Agent;
use Livewire\Attributes\On;
use Livewire\Component;

class Card extends Component
{
    public $selectedAgent;

    public $agent;

    public function mount($selectedAgent)
    {
        $this->agent = Agent::find($selectedAgent['id']);

        //        dd($selectedAgent['id']); // This will output the selected agent (if you passed it in from the parent component
        $this->selectedAgent = $selectedAgent;
    }

    #[On('agent_updated')]
    public function updateSelectedAgent($agent_id)
    {
        $this->agent = Agent::find($agent_id);
        $this->selectedAgent = [
            'id' => $this->agent->id,
            'name' => $this->agent->name,
            'description' => $this->agent->about,
            'instructions' => $this->agent->message,
        ];
    }

    public function render()
    {
        return view('livewire.agents.partials.card');
    }
}
