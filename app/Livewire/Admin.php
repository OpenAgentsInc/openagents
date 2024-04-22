<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Component;

class Admin extends Component
{
    public $totalUsers;

    public $users;

    public function mount()
    {
        if (! auth()->user() || ! auth()->user()->isAdmin()) {
            return redirect()->route('home');
        }

        $this->totalUsers = User::count();
        $this->users = User::withCount('messages')
            ->latest()
            ->get();
    }

    public function render()
    {
        return view('livewire.admin')->layout('components.layouts.admin');
    }
}
