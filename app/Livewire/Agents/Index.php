<?php

namespace App\Livewire\Agents;

use App\Models\Agent;
use Livewire\Attributes\Computed;
use Livewire\Attributes\On;
use Livewire\Attributes\Url;
use Livewire\Component;

class Index extends Component
{
    #[Url(except: '', as: 'q')]
    public $search = '';

    #[Computed]
    public function agents()
    {
        return Agent::query()->when($this->search, function ($query) {
            return $query->where('name', 'like', '%'.$this->search.'%');
        })
            ->paginate(12);
    }

    #[On('agent_deleted')]
    public function index()
    {
        $this->agents();
    }

    public function render()
    {
        return view('livewire.agents.index');
    }
}
