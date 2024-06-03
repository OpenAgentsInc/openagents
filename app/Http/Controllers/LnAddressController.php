<?php

namespace App\Http\Controllers;

use App\Models\User;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;
use Log;

class LnAddressController extends Controller
{
    private string $albyAccessToken;

    public function __construct()
    {
        $this->albyAccessToken = env('ALBY_ACCESS_TOKEN', 'none');
    }

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
            'maxSendable' => 10000000,
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

        $invoice = $this->createInvoice($amount, $descriptionHash);

        return response()->json(['pr' => $invoice]);
    }

    private function createInvoice($amount, $descriptionHash)
    {
        // Amount is msats, convert it to sats
        $amount = $amount / 1000;

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.$this->albyAccessToken,
        ])->post('https://api.getalby.com/invoices', [
            'amount' => $amount,
            'descriptionHash' => $descriptionHash,
        ]);

        if ($response->status() !== 200) {
            Log::info('Resposne status: '.$response->status());
            Log::info('Failed to create invoice: '.$response->body());

            return response()->json(['status' => 'ERROR', 'reason' => 'Failed to create invoice'], 500);
        }

        $invoice = $response->json();
        Log::info('Invoice response: '.json_encode($invoice));
        Log::info('wtf is this?: '.$invoice['payment_request']);

        return $invoice['payment_request'];
    }
}
