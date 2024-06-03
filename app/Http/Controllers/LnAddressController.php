<?php

namespace App\Http\Controllers;

use App\Models\Payin;
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
        $username = $request->query('user');

        $user = User::where('username', $username)->first();

        // If there's no user with this username, they may be a nostr user so check the name field too
        if (! $user) {
            $user = User::where('name', $username)->first();
        }

        if (! $user) {
            return response()->json(['status' => 'ERROR', 'reason' => 'User not found'], 404);
        }

        $metadata = json_encode([['text/plain', "Payment to {$username}@openagents.com"]]);

        $descriptionHash = hash('sha256', $metadata);

        // Create the invoice
        $invoice = $this->createInvoice($amount, $descriptionHash);

        // Create a Payin entry
        $payin = Payin::create([
            'user_id' => $user->id,
            'amount' => $amount,
            'payment_request' => $invoice['payment_request'],
            'payment_hash' => $invoice['payment_hash'],
            'description_hash' => $descriptionHash,
        ]);

        // Define the success action pointing to the new route
        $successAction = [
            'tag' => 'url',
            'description' => 'Received! Balance will be updated in <1 minute. Track it here:',
            'url' => url("/payin/{$payin->id}"),
        ];

        return response()->json([
            'pr' => $invoice['payment_request'],
            'successAction' => $successAction,
            'disposable' => false,
        ]);
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

        if ($response->status() !== 201) {
            Log::info('Failed to create invoice: '.$response->body());

            return response()->json(['status' => 'ERROR', 'reason' => 'Failed to create invoice'], 500);
        }

        $invoice = $response->json();

        return [
            'payment_request' => $invoice['payment_request'],
            'payment_hash' => $invoice['payment_hash'],
        ];
    }

    public function showPayinStatus($id)
    {
        $payin = Payin::findOrFail($id);

        // You can return a view with the payin information
        return view('payin-status', ['payin' => $payin]);
    }

    public function getInvoiceStatus($paymentHash)
    {
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.$this->albyAccessToken,
        ])->get("https://api.getalby.com/invoices/{$paymentHash}");

        if ($response->status() !== 200) {
            Log::info("Failed to get invoice status: {$response->body()}");

            return null;
        }

        return $response->json();
    }
}
