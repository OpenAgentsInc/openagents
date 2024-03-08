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
                'amount' => $data['amount'],
                'comment' => $data['comment'] ?? null, // Use null coalescing for optional fields
                'created_at_alby' => $data['created_at'] ?? null,
                'currency' => $data['currency'],
                'expires_at' => $data['expires_at'],
                'identifier' => $data['identifier'],
                'memo' => $data['memo'] ?? 'OpenAgents credit purchase',
                'payment_hash' => $data['payment_hash'],
                'payment_request' => $data['payment_request'],
                'settled' => $data['settled'],
                'state' => $data['state'],
                'type' => $data['type'],
                'value' => $data['value'],
                'qr_code_png' => $data['qr_code_png'],
                'qr_code_svg' => $data['qr_code_svg'],
                // Add other fields as necessary
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
