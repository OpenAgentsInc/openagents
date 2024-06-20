<?php

namespace App\Services;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Plugin;
use App\Models\User;
use App\Traits\Payable;
use Exception;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;

class PaymentService
{
    use Payable;

    private string $albyAccessToken;

    public function __construct()
    {
        $this->albyAccessToken = env('ALBY_ACCESS_TOKEN', 'none');
    }

    public function sweepAllPluginBalances()
    {
        $minAmountForLightningPayout_msats = 100 * 1000;

        $plugins = Plugin::whereHas('balances', function ($query) {
            $query->where('currency', Currency::BTC)->where('amount', '>', 0);
        })->get();

        DB::beginTransaction();
        $lightningPayOuts = [];

        try {
            foreach ($plugins as $plugin) {
                // Get plugin's BTC balance
                $balance = $plugin->balances()->where('currency', Currency::BTC)->first();

                if ($balance) {
                    $amount = $balance->amount;

                    // Distribution to plugin Author
                    $pluginAuthor = User::find($plugin->user_id);

                    // If plugin has an ln addr, use it instead
                    $pluginPaymentAddr = $plugin->payment;

                    // Calculate the distribution in msats
                    $pluginAuthorShare = 0.80 * $amount;

                    // If we need to send to a ln addr we ensure the amount is above the minimum, avoid wasting too much on fees
                    if ($pluginPaymentAddr && $pluginAuthorShare < $minAmountForLightningPayout_msats) {
                        Log::info('Skipping lightning payout for plugin '.$plugin->name.': amount too low');

                        continue;
                    }

                    // Round that down to the nearest integer number of msats, but no less than 1000 msats
                    $pluginAuthorShare = max(1000, floor($pluginAuthorShare / 1000) * 1000);

                    if ($pluginAuthor) {
                        // Withdraw balance from plugin (in msats)
                        $plugin->withdraw($amount, Currency::BTC);

                        // We store the ln payout info for later
                        if ($pluginPaymentAddr) {
                            $lightningPayOuts[] = [
                                'addr' => $pluginPaymentAddr,
                                'amount_msats' => $pluginAuthorShare,
                                'description' => "Plugin payout '$plugin->name",
                                'user' => $pluginAuthor,
                            ];
                        }

                        // Deposit balance to plugin author
                        $pluginAuthor->deposit($pluginAuthorShare, Currency::BTC);

                        // Record the payment
                        $payment = $plugin->recordPayment($pluginAuthorShare, Currency::BTC, "Swept plugin '$plugin->name' balance to author ", $plugin);

                        // Record the payment destination
                        $plugin->recordPaymentDestination($payment, $pluginAuthor);

                        // Log the payment
                        Log::info('Swept '.($pluginAuthorShare / 1000)." sats from plugin {$plugin->name} (ID {$plugin->id}) to user $pluginAuthor->name (ID {$pluginAuthor->id})");
                    } else {
                        throw new Exception('Plugin author not found');
                    }
                }
            }

            DB::commit();
        } catch (Exception $e) {
            DB::rollBack();

            // Log exception or handle it as needed
            throw $e;
        }

        // Process lightning payouts
        foreach ($lightningPayOuts as $lightningPayOut) {
            try {
                $user = $lightningPayOut['user'];
                $addr = $lightningPayOut['addr'];
                $amount = $lightningPayOut['amount_msats'];
                $description = $lightningPayOut['description'];
                $invoice = $this->getInvoiceFromLNAddress($addr, $amount / 1000, $description);
                $this->processPaymentRequest($invoice, $user);
            } catch (Exception $e) {
                Log::error('Error processing lightning payout for plugin '.$lightningPayOut['owner']->name.' ('.$lightningPayOut['owner']->id.'): '.$e->getMessage());
            }
        }

    }

    public function sweepAllAgentBalances()
    {
        // Get all agents with a non-zero balance
        $agents = Agent::whereHas('balances', function ($query) {
            $query->where('currency', Currency::BTC)->where('amount', '>', 0);
        })->get();

        DB::beginTransaction();

        try {
            foreach ($agents as $agent) {
                // Get agent's BTC balance
                $balance = $agent->balances()->where('currency', Currency::BTC)->first();

                if ($balance) {
                    $amount = $balance->amount;

                    // Calculate the distribution in msats
                    $agentAuthorShare = 0.80 * $amount;

                    // Round that down to the nearest integer number of msats, but no less than 1000 msats
                    $agentAuthorShare = max(1000, floor($agentAuthorShare / 1000) * 1000);

                    // Withdraw balance from agent (in msats)
                    $agent->withdraw($amount, Currency::BTC);

                    // Distribution to Agent Author
                    $agentAuthor = User::find($agent->user_id);

                    if ($agentAuthor) {
                        $agentAuthor->deposit($agentAuthorShare, Currency::BTC);

                        // Record the payment
                        $payment = $agent->recordPayment($agentAuthorShare, Currency::BTC, "Swept agent '$agent->name' balance to author ", $agent);

                        // Record the payment destination
                        $agent->recordPaymentDestination($payment, $agentAuthor);

                        // Log the payment
                        Log::info('Swept '.($agentAuthorShare / 1000)." sats from agent {$agent->name} (ID {$agent->id}) to user $agentAuthor->name (ID {$agentAuthor->id})");
                    } else {
                        throw new Exception('Agent author not found');
                    }
                }
            }

            DB::commit();
        } catch (Exception $e) {
            DB::rollBack();

            // Log exception or handle it as needed
            throw $e;
        }
    }

    public function getInvoiceFromLNAddress(string $addr, int $amount_sats, ?string $desc = null): string
    {
        $addrParts = explode('@', $addr);
        $identifier = $addrParts[0];
        $domain = $addrParts[1];
        $url = "https://$domain/.well-known/lnurlp/$identifier";
        $lnAddrData = Http::get($url);
        if (! $lnAddrData->ok()) {
            throw new Exception('Error fetching LN address data');
        }
        $lnAddrData = $lnAddrData->json();
        $callback = $lnAddrData['callback'];
        $amount_msats = $amount_sats * 1000;
        $callback = $callback.(strpos($callback, '?') === false ? '?' : '&').'amount='.$amount_msats;
        if ($desc) {
            $callback = $callback.'&comment='.urlencode($desc);
        }

        $invoiceData = Http::get($callback);
        if (! $invoiceData->ok()) {
            throw new Exception('Error fetching invoice data');
        }
        $invoiceData = $invoiceData->json();
        if ($invoiceData['status'] !== 'OK') {
            throw new Exception('Error fetching invoice data');
        }

        return $invoiceData['pr'];

    }

    public function payAgentForMessage(int $agentId, int $amount): bool
    {
        // amount comes in as sats, we have to convert to msats
        $amount = $amount * 1000;

        /** @var User $user */
        $user = Auth::user();

        if (! $user) {
            return false;
        }

        $agent = Agent::find($agentId);

        if (! $agent) {
            return false;
        }

        DB::beginTransaction();

        try {
            // Lock user balance for update
            DB::table('users')->where('id', $user->id)->lockForUpdate()->first();

            $currentBalance = $user->checkBalance(Currency::BTC); // Using method from Payable trait

            if ($currentBalance < $amount) {
                throw new Exception('Insufficient balance');
            }

            // Withdraw balance from user
            $user->withdraw($amount, Currency::BTC);

            // Deposit balance to agent
            $agent->deposit($amount, Currency::BTC);

            // Record the payment
            $payment = $user->recordPayment($amount, Currency::BTC, 'Payment for message', $user);

            // Record the payment destination
            $user->recordPaymentDestination($payment, $agent);

            DB::commit();

            return true;
        } catch (Exception $e) {
            //            dd($e->getMessage());
            DB::rollBack();

            return false;
        }
    }

    public function payPluginForMessage(int $pluginId, int $amount): bool
    {
        // amount comes in as sats, we have to convert to msats
        $amount = $amount * 1000;

        /** @var User $user */
        $user = Auth::user();

        if (! $user) {
            return false;
        }

        $plugin = Plugin::find($pluginId);

        if (! $plugin) {
            return false;
        }

        DB::beginTransaction();

        try {
            // Lock user balance for update
            DB::table('users')->where('id', $user->id)->lockForUpdate()->first();

            $currentBalance = $user->checkBalance(Currency::BTC); // Using method from Payable trait

            if ($currentBalance < $amount) {
                throw new Exception('Insufficient balance');
            }

            // Withdraw balance from user
            $user->withdraw($amount, Currency::BTC);

            // Deposit balance to agent
            $plugin->deposit($amount, Currency::BTC);

            // Record the payment
            $payment = $user->recordPayment($amount, Currency::BTC, 'Payment for message', $user);

            // Record the payment destination
            $user->recordPaymentDestination($payment, $plugin);

            DB::commit();

            return true;
        } catch (Exception $e) {
            //            dd($e->getMessage());
            DB::rollBack();

            return false;
        }
    }

    public function paySystemBonusToMultipleRecipients(iterable $recipients, int $amount, Currency $currency, ?string $description = 'System bonus')
    {
        DB::transaction(function () use ($recipients, $amount, $currency, $description) {
            foreach ($recipients as $recipient) {
                $recipient->payBonus($amount, $currency, $description, 'System');
            }
        });
    }

    public function processPaymentRequest($payment_request, $user = null, $onlyUnlockedFunds = false)
    {
        /** @var User $user */
        $authedUser = $user ?? Auth::user();

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
            $currentBalance = $onlyUnlockedFunds ? $authedUser->getAvailableSatsBalanceAttribute()
            : $authedUser->getSatsBalanceAttribute();

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
