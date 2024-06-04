<?php

namespace App\Livewire;

use Livewire\WithPagination;

use Livewire\Component;
use App\Models\User;
use Livewire\Attributes\Computed;
use App\Enums\UserRole;
class AdminPanel extends Component
{
    use WithPagination;


    public function checkPermissions(){
        // Redirect to the homepage if the user is not an admin
        if (! auth()->check() || auth()->user()->getRole()->value < UserRole::ADMIN->value){
            return false;
        }
        return true;
    }

    public function mount()
    {
        if(!$this->checkPermissions()){
           return  redirect()->route('home');
        }
    }

    private function totalUsers()
    {
        return User::count();
    }


    public function render()
    {
        if(!$this->checkPermissions()){
            return;
        }

        $users = User::paginate(10); // Fetch 10 users per page

        return view('livewire.admin', [
            'users' => $users,
            'totalUsers' => $this->totalUsers(),
        ]);
    }
}
