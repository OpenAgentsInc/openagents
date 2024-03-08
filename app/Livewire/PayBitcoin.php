<?php

namespace App\Livewire;

use Illuminate\Support\Facades\Http;
use Livewire\Component;

class PayBitcoin extends Component
{
    public $qr;

    public function mount()
    {
        // Create invoice by hitting the Alby API - via POST to https://api.getalby.com/invoices
        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.env('ALBY_ACCESS_TOKEN'),
        ])->post('https://api.getalby.com/invoices', [
            'amount' => 100,
            'description' => 'Payment for the product',
        ]);

        $this->qr = $response->json()['qr_code_png'];
    }

    public function render()
    {
        return view('livewire.pay-bitcoin');
    }
}
