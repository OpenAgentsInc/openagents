<?php

namespace App\Livewire\Modals;

use App\Services\PaymentService;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Deposit extends ModalComponent
{
    use LivewireAlert;

    public $amount;

    public $max_deposit_sats = 10000;

    public $min_deposit_sats = 1;

    public static function modalMaxWidth(): string
    {
        return '3xl';
    }

    public function render()
    {
        return view('livewire.modals.deposit');
    }

    protected $rules = [
        'amount' => 'required|integer',
    ];

    public function mount()
    {
        // If the user is not logged in, redirect to the homepage
        if (! auth()->check()) {
            redirect()->route('home');

            return;
        }
        $this->balance_sats = auth()->user()->getAvailableSatsBalanceAttribute();
    }

    public function submitDepositRequest(PaymentService $paymentService): void
    {
        if (! auth()->check()) {
            redirect()->route('home');

            return;
        }
        $this->validate();

        $amount = $this->amount;

        if ($amount < $this->min_deposit_sats) {
            $this->alert('error', 'Minimum deposit amount is '.$this->min_deposit_sats.' sats');

            return;
        }
        if ($amount > $this->max_deposit_sats) {
            $this->alert('error', 'Maximum deposit amount is '.$this->max_deposit_sats.' sats');

            return;
        }

        $user = auth()->user();
        $addr = $user->getLightningAddress();
        try {
            $invoice = $paymentService->getInvoiceFromLNAddress($addr, $amount, 'Deposit');
            $this->js('setTimeout(() => { Livewire.dispatch("openModal", { component: "modals.invoice", arguments:{ invoice: "'.$invoice.'"  }}) }, 100)');
        } catch (\Exception $e) {
            $this->alert('error', $e->getMessage());

            return;
        }

    }
}
