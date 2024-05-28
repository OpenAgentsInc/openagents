<?php

namespace App\Traits;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
use App\Models\Payment;
use App\Models\PaymentDestination;
use App\Models\PaymentSource;
use App\Models\User;
use Exception;
use Illuminate\Support\Facades\DB;

trait Payable
{
    public function multipay(array $recipients): void
    {
        DB::transaction(function () use ($recipients) {
            foreach ($recipients as $recipient) {
                [$payee, $amount, $currency] = $recipient;
                $this->withdraw($amount, $currency);
                $payee->deposit($amount, $currency);
                $payment = $this->recordPayment($amount, $currency);
                $this->recordPaymentDestination($payment, $payee);
            }
        });
    }

    public function withdraw(int $amount, Currency $currency)
    {
        $balance = $this->balances()->where('currency', $currency->value)->firstOrFail();
        if ($balance->amount < $amount) {
            throw new Exception('Insufficient balance');
        }
        $balance->amount -= $amount;
        $balance->save();
    }

    public function balances()
    {
        return $this->morphMany(Balance::class, 'holder');
    }

    public function deposit(int $amount, Currency $currency)
    {
        $balance = $this->balances()->firstOrCreate(
            ['currency' => $currency->value],
            ['amount' => 0]
        );
        $balance->amount += $amount;
        $balance->save();
    }

    private function recordPayment(int $amount, Currency $currency, ?string $description = null)
    {
        $payment = Payment::create([
            'payer_type' => get_class($this),
            'payer_id' => $this->id,
            'currency' => $currency->value,
            'amount' => $amount,
            'description' => $description,
        ]);

        $this->recordPaymentSource($payment);

        return $payment;
    }

    private function recordPaymentSource(Payment $payment)
    {
        PaymentSource::create([
            'payment_id' => $payment->id,
            'source_type' => get_class($this),
            'source_id' => $this->id,
        ]);
    }

    private function recordPaymentDestination(Payment $payment, $payee)
    {
        PaymentDestination::create([
            'payment_id' => $payment->id,
            'destination_type' => get_class($payee),
            'destination_id' => $payee->id,
        ]);
    }

    public function payments()
    {
        return $this->morphMany(Payment::class, 'payer');
    }

    public function payAgent(Agent $agent, int $amount, Currency $currency)
    {
        DB::transaction(function () use ($agent, $amount, $currency) {
            $this->withdraw($amount, $currency);
            $agent->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency);
            $this->recordPaymentDestination($payment, $agent);
        });
    }

    public function payUser(User $user, int $amount, Currency $currency)
    {
        DB::transaction(function () use ($user, $amount, $currency) {
            $this->withdraw($amount, $currency);
            $user->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency);
            $this->recordPaymentDestination($payment, $user);
        });
    }

    public function checkBalance(Currency $currency)
    {
        $balance = $this->balances()->where('currency', $currency->value)->first();

        return $balance ? $balance->amount : 0;
    }

    public function getBalanceAttribute()
    {
        return $this->balances()->get();
    }

    public function newBalance(int $amount, Currency $currency)
    {
        $this->balances()->create([
            'currency' => $currency->value,
            'amount' => $amount,
        ]);
    }

    public function payBonus(int $amount, Currency $currency, ?string $description = 'System bonus')
    {
        DB::transaction(function () use ($amount, $currency, $description) {
            $this->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency, $description);
            $this->recordPaymentDestination($payment, $this);
        });
    }
}
