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
use InvalidArgumentException;

trait Payable
{
    // Add an attribute sats_earned that calculates all bitcoin payments received
    public function getSatsEarnedAttribute()
    {
        return $this->receivedPayments()->where('currency', Currency::BTC)->sum('amount');
    }

    public function receivedPayments()
    {
        return $this->hasManyThrough(
            Payment::class,
            PaymentDestination::class,
            'destination_id', // Foreign key on PaymentDestination table
            'id', // Foreign key on Payment table
            'id', // Local key on User table
            'payment_id'  // Local key on PaymentDestination table
        );
    }

    public function multipay(array $recipients): void
    {
        DB::transaction(function () use ($recipients) {
            foreach ($recipients as $recipient) {
                [$payee, $amount, $currency] = $recipient;
                $this->withdraw($amount, $currency);
                $payee->deposit($amount, $currency);
                $payment = $this->recordPayment($amount, $currency, null, $this); // Ensure payer is 'this'
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

    public function recordPayment(int $amount, Currency $currency, ?string $description = null, $payer = null)
    {
        if (is_string($payer) && $payer === 'System') {
            $payerType = 'System';
            $payerId = 0;
        } elseif (is_object($payer)) {
            $payerType = get_class($payer);
            $payerId = $payer->id;
        } else {
            throw new InvalidArgumentException('Payer must be an object or the string "System"');
        }

        $payment = Payment::create([
            'payer_type' => $payerType,
            'payer_id' => $payerId,
            'currency' => $currency->value,
            'amount' => $amount,
            'description' => $description,
        ]);

        $this->recordPaymentSource($payment, $payer);

        return $payment;
    }

    public function recordPaymentSource(Payment $payment, $payer = null)
    {
        if (is_string($payer) && $payer === 'System') {
            $sourceType = 'System';
            $sourceId = 0;
        } elseif (is_object($payer)) {
            $sourceType = get_class($payer);
            $sourceId = $payer->id;
        } else {
            throw new InvalidArgumentException('Payer must be an object or the string "System"');
        }

        PaymentSource::create([
            'payment_id' => $payment->id,
            'source_type' => $sourceType,
            'source_id' => $sourceId,
        ]);
    }

    public function recordPaymentDestination(Payment $payment, $payee)
    {
        PaymentDestination::create([
            'payment_id' => $payment->id,
            'destination_type' => get_class($payee),
            'destination_id' => $payee->id,
        ]);
    }

    public function sentPayments()
    {
        return $this->morphMany(Payment::class, 'payer');
    }

    public function payAgent(Agent $recipient, int $amount, Currency $currency, ?string $description = 'Payment')
    {
        DB::transaction(function () use ($recipient, $amount, $currency, $description) {
            $this->withdraw($amount, $currency);
            $recipient->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency, $description, $this); // Ensure payer is 'this'
            $this->recordPaymentDestination($payment, $recipient);
        });
    }

    public function payUser(User $recipient, int $amount, Currency $currency, ?string $description = 'Payment')
    {
        DB::transaction(function () use ($recipient, $amount, $currency, $description) {
            $this->withdraw($amount, $currency);
            $recipient->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency, $description, $this); // Ensure payer is 'this'
            $this->recordPaymentDestination($payment, $recipient);
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

    public function payBonus(int $amount, Currency $currency, ?string $description = 'System bonus', $payer = 'System')
    {
        DB::transaction(function () use ($amount, $currency, $description, $payer) {
            $this->deposit($amount, $currency);
            $payment = $this->recordPayment($amount, $currency, $description, $payer);
            $this->recordPaymentDestination($payment, $this);
        });
    }
}
