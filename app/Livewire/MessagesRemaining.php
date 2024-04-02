<?php

namespace App\Livewire;

use App\Models\Message;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class MessagesRemaining extends Component
{
    public $remaining = 0;

    public function mount()
    {
        $this->calculateRemainingMessages();
    }

    protected function calculateRemainingMessages()
    {
        if (Auth::check()) {
            // User is authenticated
            $userId = Auth::id();
            // Assuming a 'user_id' column in your messages table for simplicity
            $messagesToday = Message::where('user_id', $userId)
                ->whereDate('created_at', today())
                ->count();

            // Adjust these numbers as per your actual limits
            $initialFreeMessages = 10;
            $additionalMessagesAfterSignup = 50;

            $totalAvailable = $initialFreeMessages + $additionalMessagesAfterSignup;
            $this->remaining = max(0, $totalAvailable - $messagesToday);

        } else {
            // User is not authenticated, use session ID
            $sessionId = Session::getId();
            // You'll need a way to associate messages with sessions for unauthenticated users
            // This might require a custom implementation
            $messagesToday = Message::where('session_id', $sessionId)
                ->whereDate('created_at', today())
                ->count();

            // Assuming 10 free messages for unauthenticated users
            $this->remaining = max(0, 10 - $messagesToday);
        }
    }

    #[On('message-created')]
    public function updateStuff()
    {
        $this->calculateRemainingMessages();
    }

    public function render()
    {
        return view('livewire.messages-remaining');
    }
}
