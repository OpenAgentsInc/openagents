<?php

namespace App\Livewire;

use App\Models\Payment;
use Carbon\Carbon;
use Livewire\Component;

class PrismDashboard extends Component
{
    public $payments = [];

    protected $listeners = ['echo:payments,PaymentCreated' => 'refreshPayments'];

    public function mount()
    {
        $this->refreshPayments();
    }

    public function refreshPayments()
    {
        $this->payments = Payment::with('receiver')
            ->latest()
            ->get()
            ->map(function ($payment) {
                return [
                    'id' => $payment->prism_id,
                    'createdAt' => Carbon::createFromTimestamp($payment->prism_created_at)->toDateTimeString(),
                    'updatedAt' => Carbon::createFromTimestamp($payment->prism_updated_at)->toDateTimeString(),
                    'expiresAt' => Carbon::createFromTimestamp($payment->expires_at)->toDateTimeString(),
                    'senderId' => $payment->sender_prism_id,
                    'receiverId' => $payment->receiver_prism_id,
                    'receiverAddress' => $payment->receiver->ln_address ?? 'N/A',
                    'amountMsat' => $payment->amount_msat,
                    'status' => $payment->status,
                    'resolvedAt' => $payment->resolved_at ? Carbon::createFromTimestamp($payment->resolved_at)->toDateTimeString() : null,
                    'resolved' => $payment->resolved,
                    'prismPaymentId' => $payment->prism_payment_id,
                    'bolt11' => $payment->bolt11,
                    'preimage' => $payment->preimage,
                    'failureCode' => $payment->failure_code,
                    'type' => $payment->type,
                ];
            })->toArray();
    }

    public function render()
    {
        return view('livewire.prism-dashboard');
    }
}
