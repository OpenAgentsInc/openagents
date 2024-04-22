<?php

namespace App\Livewire;

use App\Models\User;
use App\Services\PrismService;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\On;
use Livewire\Component;

class Admin extends Component
{
    use LivewireAlert;

    public $totalUsers;

    public $users;

    public $selectedUserIds = [];

    #[On('toggleUserId')]
    public function toggleUserId($id)
    {
        if (in_array($id, $this->selectedUserIds)) {
            $this->selectedUserIds = array_diff($this->selectedUserIds, [$id]);
        } else {
            $this->selectedUserIds[] = $id;
        }
    }

    public function payMultiple()
    {
        $prism = new PrismService();

        foreach ($this->selectedUserIds as $userId) {
            $user = User::find($userId);

            // Does the user have a prism_id
            if (! $user->prism_id) {
                //                $this->alert('error', 'User '.$user->id.' does not have a prism_id');

                // Create a user in Prism
                $response = $prism->createUser($user->lightning_address);

                dd($response);

                if (isset($response['id'])) {
                    $user->prism_id = $response['id'];
                    $user->save();
                } else {
                    $this->alert('error', 'Failed to create user in Prism');

                    return;
                }

            }
        }

    }

    public function deleteMultiple()
    {
        foreach ($this->selectedUserIds as $userId) {
            $this->delete($userId, false);
        }
        $this->setUsers();
        $this->alert('success', 'Deleted '.count($this->selectedUserIds).' users');
        $this->selectedUserIds = [];
    }

    public function delete($userId, $set = true)
    {
        $user = User::find($userId);
        $user->messages()->delete();
        $user->threads()->delete();
        $user->delete();
        if ($set) {
            $this->setUsers();
        }
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
