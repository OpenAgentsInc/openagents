<?php

namespace App\Services;

use App\Enums\Currency;
use App\Models\User;
use Exception;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Log;

class PaymentService
{
    private string $albyAccessToken;

    public function __construct()
    {
        $this->albyAccessToken = env('ALBY_ACCESS_TOKEN');
    }

    public function paySystemBonusToMultipleRecipients(iterable $recipients, int $amount, Currency $currency, ?string $description = 'System bonus')
    {
        DB::transaction(function () use ($recipients, $amount, $currency, $description) {
            foreach ($recipients as $recipient) {
                $recipient->payBonus($amount, $currency, $description, 'System');
            }
        });
    }

    public function processPaymentRequest($payment_request)
    {
        /** @var User $user */
        $authedUser = Auth::user();

        if (! $authedUser) {
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
            DB::table('users')->where('id', $authedUser->id)->lockForUpdate()->first();
            $currentBalance = $authedUser->getSatsBalanceAttribute();

            Log::info("Current sats balance: $currentBalance\n");
            Log::info("Amount to pay: $amount\n");

            if ($currentBalance < $amount) {
                throw new Exception('Insufficient balance');
            }

            DB::table('payments')->insert([
                'payer_type' => get_class($authedUser),
                'payer_id' => $authedUser->id,
                'currency' => Currency::BTC,
                'amount' => $amount,
                'metadata' => json_encode($metadata),
                'description' => 'Payment request for '.$payment_request,
                'created_at' => now(),
                'updated_at' => now(),
            ]);

            DB::table('balances')->where([
                ['holder_type', '=', get_class($authedUser)],
                ['holder_id', '=', $authedUser->id],
                ['currency', '=', Currency::BTC],
            ])->update([
                'amount' => DB::raw('amount - '.$amount * 1000),
                'updated_at' => now(),
            ]);

            DB::commit();

            $payResponse = Http::withHeaders([
                'Content-Type' => 'application/json',
                'Authorization' => 'Bearer '.$this->albyAccessToken,
            ])->post('https://api.getalby.com/payments/bolt11', [
                'invoice' => $payment_request,
                'description' => 'test withdrawal',
            ]);

            if ($payResponse->ok()) {
                DB::table('payments')->where('metadata->payment_hash', $payment_hash)
                    ->update([
                        'metadata->invoice_status' => 'settled',
                        'updated_at' => now(),
                    ]);
            }

            return ['ok' => true, 'response' => $payResponse->json()];
        } catch (Exception $e) {
            DB::rollBack();

            return ['ok' => false, 'error' => $e->getMessage()];
        }
    }
}
