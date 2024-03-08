<?php

namespace App\Livewire;

use App\Models\Invoice;
use Illuminate\Support\Facades\Http;
use Livewire\Component;

class PayBitcoin extends Component
{
    public $amount = 100;

    public $qr;

    public function generateInvoice()
    {
        // Create invoice by hitting the Alby API - via POST to https://api.getalby.com/invoices
        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.env('ALBY_ACCESS_TOKEN'),
        ])->post('https://api.getalby.com/invoices', [
            'amount' => $this->amount,
            'description' => 'OpenAgents credit purchase',
        ]);

        if ($response->successful()) {
            $data = $response->json();

            // Save the invoice details to the database
            Invoice::create([
                'settled' => $data['settled'] ?? false,
                'amount' => $data['amount'],
                'comment' => $data['comment'] ?? null,
                'created_at_alby' => $data['created_at'] ?? null,
                'creation_date' => $data['creation_date'] ?? null,
                'currency' => $data['currency'],
                'destination_pubkey' => $data['destination_pubkey'] ?? null,
                'expiry' => $data['expiry'] ?? null,
                'fiat_currency' => $data['fiat_currency'] ?? null,
                'fiat_in_cents' => $data['fiat_in_cents'] ?? null,
                'identifier' => $data['identifier'],
                'memo' => $data['memo'] ?? null,
                'metadata' => $data['metadata'] ?? null,
                'payer_name' => $data['payer_name'] ?? null,
                'payer_email' => $data['payer_email'] ?? null,
                'payer_pubkey' => $data['payer_pubkey'] ?? null,
                'payment_hash' => $data['payment_hash'],
                'payment_request' => $data['payment_request'],
                'preimage' => $data['preimage'] ?? null,
                'r_hash_str' => $data['r_hash_str'] ?? null,
                'state' => $data['state'],
                'type' => $data['type'],
                'qr_code_png' => $data['qr_code_png'],
                'qr_code_svg' => $data['qr_code_svg'],
                'value' => $data['value'],
                'settled_at' => $data['settled_at'] ?? null,
                'expires_at' => $data['expires_at'],
            ]);

            $this->qr = $data['qr_code_png'];
        } else {
            // Handle API request failure (e.g., log the error, show a message to the user)
            $this->qr = null;
        }
    }

    public function render()
    {
        return view('livewire.pay-bitcoin');
    }
}
