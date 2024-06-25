<?php

namespace App\Livewire\Modals;

use App\Services\PaymentService;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Withdraw extends ModalComponent
{
    use LivewireAlert;

    public $payment_request;

    public $balance_sats;

    public static function modalMaxWidth(): string
    {
        return '3xl';
    }

    public function render()
    {
        return view('livewire.modals.withdraw');
    }

    protected $rules = [
        'payment_request' => 'required|string',
    ];

    public function mount()
    {
        // If the user is not logged in, redirect to the homepage
        if (! auth()->check()) {
            return redirect()->route('home');
        }
        $this->balance_sats = auth()->user()->getAvailableSatsBalanceAttribute();
    }

    public function submitPaymentRequest(PaymentService $paymentService): void
    {
        if (! auth()->check()) {
            redirect()->route('home');

            return;
        }
        $this->validate();

        $response = $paymentService->processPaymentRequest($this->payment_request, null, true);

        if ($response['ok']) {
            $this->alert('success', 'Payment processed successfully');

        } else {
            $this->alert('error', $response['error'] ?? 'Something went wrong.');
        }

        // Optionally update the balance and payins after processing the payment
        $user = auth()->user()->fresh();
        $this->balance_btc = $user->getAvailableSatsBalanceAttribute();
        $this->payins = $user->payins()->get()->reverse();
        // close modal
        $this->closeModal();
    }
}
