<?php

namespace App\Livewire;

use App\Models\User;
use App\Services\PaymentService;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;

class WalletScreen extends Component
{
    use LivewireAlert;

    public $balance_btc = 1;

    public $pending_balance_btc = 0;

    public $payment_request;

    public $received_payments;

    public $lightning_address;

    public $payins; // Add this line

    public $custom_lightning_address;

    public $lightning_domain;

    public $address_history;

    protected $rules = [
        'payment_request' => 'required|string',
    ];

    // On mount, grab the user's bitcoin balance and payins
    public function mount()
    {
        // If the user is not logged in, redirect to the homepage
        if (! auth()->check()) {
            return redirect()->route('home');
        }

        /** @var User $user */
        $user = auth()->user();
        $this->balance_btc = $user->getAvailableSatsBalanceAttribute();

        $agentsBalance = $user->agents->sum('sats_balance');
        $pluginBalance = $user->plugins->sum('sats_balance');
        $this->pending_balance_btc = $agentsBalance + $pluginBalance;

        $this->received_payments = $user->receivedPayments()->get()->reverse();

        $this->lightning_address = $user->getLightningAddress();
        $this->custom_lightning_address = explode('@', $this->lightning_address)[0];
        $this->lightning_domain = explode('@', $this->lightning_address)[1];

        $this->address_history = $user->lightningAddresses()->pluck('address')->toArray();
        // Fetch the payins
        $this->payins = $user->payins()->get()->reverse(); // Assumes there's a payins() relationship
    }

    public function submitPaymentRequest(PaymentService $paymentService): void
    {
        $this->validate();

        $response = $paymentService->processPaymentRequest($this->payment_request, null, true);

        if ($response['ok']) {
            session()->flash('message', 'Payment processed successfully.');
        } else {
            session()->flash('error', $response['error'] ?? 'Something went wrong.');
        }

        // Optionally update the balance and payins after processing the payment
        $user = auth()->user()->fresh();
        $this->balance_btc = $user->getAvailableSatsBalanceAttribute();
        $this->payins = $user->payins()->get()->reverse();
    }

    public function withdraw()
    {
        $this->js('setTimeout(() => { Livewire.dispatch("openModal", { component: "modals.withdraw" }) }, 100)');
    }

    public function deposit()
    {
        $this->js('setTimeout(() => { Livewire.dispatch("openModal", { component: "modals.deposit" }) }, 100)');
    }

    public function render()
    {
        return view('livewire.wallet-screen');
    }

    public function updateCustomLightningAddress()
    {
        try {
            $user = auth()->user();
            if (! $user->isPro()) {
                $this->alert('error', 'You must be a pro user to set a custom lightning address');

                return;
            }
            $user->setVanityAddress($this->custom_lightning_address);
            $this->alert('success', 'Updated Lightning address');

            return redirect()->route('wallet');
        } catch (\Exception $e) {
            $this->alert('error', $e->getMessage());
        }
    }
}
