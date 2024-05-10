<?php

namespace App\Livewire;

use App\Models\Agent;
use Livewire\Component;

class FeaturedAgents extends Component
{
    public $agents;

    //    public function mount()
    //    {
    //        //        $this->agents = Agent::where('featured', true)->get();
    //        $this->agents = Agent::withCount(['threads as unique_threads_count'])->get();
    //
    //    }

    public function mount()
    {
        $this->agents = Agent::withCount(['threads as unique_threads_count'])
            ->get()
            ->each(function ($agent) {
                $agent->unique_users_count = $agent->getUniqueUsersCountAttribute();
            });
    }

    public function render()
    {
        return view('livewire.featured-agents');
    }
}
