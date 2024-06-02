<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;

class LnAddressController extends Controller
{
    public function handleLnurlp($user)
    {
        dd($user);

        if (! $lnAddress) {
            return response()->json(['status' => 'ERROR', 'reason' => 'User not found'], 404);
        }

        $callbackUrl = url("/lnurlp/callback?user={$user}");
        $metadata = json_encode([['text/plain', "Payment to {$user}@openagents.com"]]);

        $response = [
            'callback' => $callbackUrl,
            'maxSendable' => 1000000000,
            'minSendable' => 1000,
            'metadata' => $metadata,
            'commentAllowed' => 240,
            'tag' => 'payRequest',
        ];

        return response()->json($response);
    }

    public function handleCallback(Request $request)
    {
        $amount = $request->query('amount');
        $user = $request->query('user');
        $metadata = json_encode([['text/plain', "Payment to {$user}@openagents.com"]]);

        $descriptionHash = hash('sha256', $metadata);
        // Assuming you have a method to create an invoice via Voltage
        $invoice = $this->createInvoice($amount, $descriptionHash);

        return response()->json(['pr' => $invoice]);
    }

    private function createInvoice($amount, $descriptionHash)
    {
        // Example of creating an invoice with Voltage API
        // Replace with actual implementation
    }
}
