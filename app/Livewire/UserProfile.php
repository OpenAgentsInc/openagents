<?php

namespace App\Livewire;

use App\Models\User;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;

class UserProfile extends Component
{
    use LivewireAlert;

    // protected $listeners = ['change-role' => 'changeRole'];

    public $user;

    public $assignableRolesByViewer;

    public $viewerCanModerate;

    private function getUser($username)
    {
        $user = User::where('username', $username)->first();
        if (! $user) {
            $user = User::where('name', $username)->firstOrFail();
        }
        if (! $user) {
            $this->alert('error', 'User not found');

            return;
        }

        return $user;
    }

    private function getCanModerate($currentUser, $user)
    {
        return $currentUser->username !== $user->username && // user cannot change its own role
            $currentUser->getRole()->canModerate($user->getRole()); // user cannot change role of higher role
    }

    private function getAssignableRoles($currentUser)
    {
        $assignableRoles = $currentUser->getRole()->getAssignableRoles();

        return $assignableRoles;
    }

    public function mount($username)
    {
        $this->user = $this->getUser($username);

        $currentUser = auth()->user();
        $this->viewerCanModerate = $this->getCanModerate($currentUser, $this->user);
        $this->assignableRolesByViewer = $this->getAssignableRoles($currentUser);
    }

    public function handleChange($role)
    {

        $role = intval($role);
        $username = $this->user->username ?? $this->user->name;

        $currentUser = auth()->user();
        if (! $currentUser) {
            $this->alert('error', 'You must be logged in to change user role');

            return;
        }

        $this->user = $this->getUser($username);
        if (! $this->user) {
            $this->alert('error', 'User not found');

            return;
        }

        $this->viewerCanModerate = $this->getCanModerate($currentUser, $this->user);
        $this->assignableRolesByViewer = $this->getAssignableRoles($currentUser);

        if (! $this->viewerCanModerate) {
            $this->alert('error', 'You do not have permission to change this user role');

            return;
        }

        if (! in_array($role, array_map(function ($role) {
            return $role->value;
        }, $this->assignableRolesByViewer))) {

            $this->alert('error', 'Invalid role '.$role);

            return;
        }

        $this->user->role = $role;
        $this->user->save();
        $this->alert('success', 'Role changed successfully');
    }

    public function render()
    {
        return view('livewire.user-profile');
    }
}
