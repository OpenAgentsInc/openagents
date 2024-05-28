<?php

// app/Services/PaymentService.php

namespace App\Services;

use App\Enums\Currency;
use Exception;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;

class PaymentService
{
    private string $albyAccessToken;

    // in constructor set alby access token
    public function __construct()
    {
        $this->albyAccessToken = env('ALBY_ACCESS_TOKEN');
    }

    public function processPaymentRequest($payment_request)
    {
        $user = Auth::user();

        if (! $user) {
            return response()->json(['message' => 'Unauthorized'], 401);
        }

        // Get payment details from Alby
        $albyResponse = Http::get("https://api.getalby.com/decode/bolt11/{$payment_request}");

        if (! $albyResponse->ok()) {
            return ['ok' => false, 'message' => 'Invalid invoice'];
        }

        $albyData = $albyResponse->json();
        $payment_hash = $albyData['payment_hash'];
        $expires_at = $albyData['created_at'] + $albyData['expiry'];
        $amount = $albyData['amount'];
        $metadata = [
            'expires_at' => $expires_at,
            'payment_hash' => $payment_hash,
            'payment_request' => $payment_request,
        ];

        DB::beginTransaction();

        try {
            // Check balance and lock the row for the current transaction
            $user = DB::table('users')->where('id', $user->id)->lockForUpdate()->first();
            $currentBalance = $user->balance;

            if ($currentBalance < $amount) {
                throw new Exception('Insufficient balance');
            }

            // Insert into Payments table
            DB::table('payments')->insert([
                'payer_type' => get_class($user),
                'payer_id' => $user->id,
                'currency' => Currency::BTC,
                'amount' => $amount,
                'metadata' => json_encode($metadata),
                'description' => 'Payment request for '.$payment_request,
                'created_at' => now(),
                'updated_at' => now(),
            ]);

            // Update balance
            DB::table('users')->where('id', $user->id)->update([
                'balance' => $currentBalance - $amount,
            ]);

            // Commit the transaction
            DB::commit();

            $payResponse = Http::withHeaders([
                'Content-Type' => 'application/json',
                'Authorization' => 'Bearer '.$this->albyAccessToken,
            ])->post('https://api.getalby.com/payments/bolt11', [
                'invoice' => $payment_request,
            ]);

            // If payment is successful, update the payment record by setting invoice_status to settled
            if ($payResponse->ok()) {
                DB::table('payments')->where('metadata->payment_hash', $payment_hash)
                    ->update([
                        'metadata->invoice_status' => 'settled',
                        'updated_at' => now(),
                    ]);
            }

            return ['ok' => true];
        } catch (Exception $e) {
            DB::rollBack();

            return ['ok' => false, 'error' => $e->getMessage()];
        }
    }
}
