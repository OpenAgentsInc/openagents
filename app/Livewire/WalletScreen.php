<?php

namespace App\Livewire;

use App\Models\User;
use App\Services\PaymentService;
use Livewire\Component;

class WalletScreen extends Component
{
    public $balance_btc = 1;

    public $payment_request;

    protected $rules = [
        'payment_request' => 'required|string',
    ];

    // On mount, grab the user's bitcoin balance
    public function mount()
    {
        // If the user is not logged in, redirect to the homepage
        if (! auth()->check()) {
            return redirect()->route('home');
        }

        /** @var User $user */
        $user = auth()->user();
        $this->balance_btc = $user->getSatsBalanceAttribute();
    }

    public function submitPaymentRequest(PaymentService $paymentService): void
    {
        $this->validate();

        $response = $paymentService->processPaymentRequest($this->payment_request);

        if ($response['ok']) {
            session()->flash('message', 'Payment processed successfully.');
        } else {
            session()->flash('error', $response['error'] ?? 'Something went wrong.');
        }

        // Optionally update the balance after processing the payment
        $this->balance_btc = auth()->user()->fresh()->getSatsBalanceAttribute();
    }

    public function render()
    {
        return view('livewire.wallet-screen');
    }
}
