<?php

namespace App\Livewire;

use App\Models\User;
use App\Services\PrismService;
use Exception;
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

        $recipients = [];

        foreach ($this->selectedUserIds as $userId) {
            $user = User::find($userId);

            // If user doesn't have a lightning address, go byebye
            if (! $user->lightning_address) {
                $this->alert('error', $user->name.' does not have a lightning address');

                return;
            }

            // If the user doesn't have a prism user ID, let's make one for them
            if (! $user->prism_user_id) {
                // Create a user in Prism
                $response = $prism->createUser($user->lightning_address);

                if (isset($response['id'])) {
                    $user->prism_user_id = $response['id'];
                    $user->save();
                    $this->alert('success', 'Created user in Prism');
                } else {
                    $this->alert('error', 'Failed to create user in Prism');

                    return;
                }
            }
            $recipients[] = [$user->prism_user_id, 1]; // assume equal weight for now
        }

        if (empty($recipients)) {
            $this->alert('error', 'No valid users');

            return;
        }

        try {
            $amount = 50; // example amount in sats
            $prism->sendPayment($amount, $recipients);
            $this->alert('success', 'Payment sent successfully');
        } catch (Exception $e) {
            $this->alert('error', 'Failed to send payment');
            dump($e->getMessage());
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
