<?php

namespace App\Http\Controllers;

use App\Models\User;
use Illuminate\Http\Request;

class LnAddressController extends Controller
{
    public function handleLnurlp($username)
    {
        // First see if there is a user with this username
        $user = User::where('username', $username)->first();

        if (! $user) {
            return response()->json(['status' => 'ERROR', 'reason' => 'User not found'], 404);
        }

        $callbackUrl = url("/lnurlp/callback?user={$username}");
        $metadata = json_encode([['text/plain', "Test! Payment to {$username}@openagents.com"]]);

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
