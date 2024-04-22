<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Component;

class Admin extends Component
{
    public $totalUsers;

    public $users;

    public function delete($userId)
    {
        $user = User::find($userId);
        $user->messages()->delete();
        $user->threads()->delete();
        $user->delete();
        $this->setUsers();
    }

    private function setUsers()
    {
        $this->users = User::withCount('messages')
            ->latest()
            ->get();
        $this->totalUsers = User::count();
    }

    public function mount()
    {
        if (! auth()->user() || ! auth()->user()->isAdmin()) {
            return redirect()->route('home');
        }

        $this->setUsers();
    }

    public function render()
    {
        return view('livewire.admin')->layout('components.layouts.admin');
    }
}
