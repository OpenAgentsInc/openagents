<?php

namespace App\Livewire;

use App\Models\Agent;
use Illuminate\Support\Facades\DB;
use Livewire\Attributes\Computed;
use Livewire\Component;

class PopularAgents extends Component
{
    
    #[Computed]
    public function agents()
    {
        return Agent::query()
            ->withCount(['messages as unique_users_count' => function ($query) {
                $query->distinct('user_id');
            }])
            ->withCount('messages as thread_count')
            ->orderByDesc(function ($query) {
                $query->select(DB::raw('unique_users_count * 3 + thread_count'));
            })
            ->limit(10)
            ->get();
    }

    public function render()
    {
        return view('livewire.popular-agents');
    }
}
