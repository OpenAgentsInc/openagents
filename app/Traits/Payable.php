<?php

namespace App\Traits;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
use App\Models\LockedBalance;
use App\Models\Payment;
use App\Models\PaymentDestination;
use App\Models\PaymentSource;
use App\Models\User;
use Exception;
use Illuminate\Support\Facades\DB;
use InvalidArgumentException;

trait Payable
{
    public function getSatsEarnedAttribute(): int
    {
        return (int) $this->receivedPayments()->where('currency', Currency::BTC)->sum('amount') / 1000;
    }

    public function receivedPayments()
    {
        return $this->morphMany(PaymentDestination::class, 'destination')
            ->leftJoin('payments', 'payment_destinations.payment_id', '=', 'payments.id');
    }

    private function getLockedMSats()
    {
        $this->lockedBalances()
            ->where('currency', Currency::BTC)
            ->where('expires_at', '<', now())
            ->delete();

        $lockedBalances = $this->lockedBalances()
            ->where('currency', Currency::BTC)
            ->where('expires_at', '>', now())
            ->lockForUpdate();

        if ($lockedBalances->count() == 0) {
            return 0;
        }

        $lockedBalanceAmmount = $lockedBalances->sum('amount');
        if ($lockedBalanceAmmount < 0) {
            $lockedBalanceAmmount = 0;
        }

        return $lockedBalanceAmmount;

    }

    public function getSatsBalanceAttribute(): int
    {

        $balance = $this->balances()->where('currency', Currency::BTC)->lockForUpdate()->first();

        return $balance ? (int) ($balance->amount / 1000) : 0;
    }

    public function getAvailableSatsBalanceAttribute(): int
    {
        return DB::transaction(function () {
            $balance = $this->balances()->where('currency', Currency::BTC)->lockForUpdate()->first();
            if (! $balance) {
                return 0;
            }
            $lockedBalanceAmountSats = (int) ($this->getLockedMSats() / 1000);

            return (int) ($balance->amount / 1000) - $lockedBalanceAmountSats;
        });
    }

    public function getLockedSatsBalanceAttribute(): int
    {
        return DB::transaction(function () {
            $lockedBalanceAmountSats = (int) ($this->getLockedMSats() / 1000);

            return $lockedBalanceAmountSats;
        });
    }

    public function unlockSats(int $lockId)
    {
        DB::transaction(function () use ($lockId) {
            $lock = $this->lockedBalances()->where('id', $lockId)->lockForUpdate()->first();
            if ($lock) {
                $lock->delete();
            }
        });
    }

    public function lockSats(int $amount): int
    {
        return DB::transaction(function () use ($amount) {
            $amount_msats = $amount * 1000;
            $balance = $this->balances()->where('currency', Currency::BTC)->lockForUpdate()
                ->firstOrFail();
            $lockedBalances = $this->lockedBalances()->where('currency', Currency::BTC)->lockForUpdate();
            $lockedBalanceAmmount = $lockedBalances->sum('amount');
            if (($balance->amount - $lockedBalanceAmmount) < $amount_msats) {
                throw new Exception('Insufficient balance');
            }
            // create new locked balance
            $lock = $this->lockedBalances()->create([
                'currency' => Currency::BTC,
                'amount' => $amount_msats,
                'expires_at' => now()->addMinutes(15),
            ]);

            return $lock->id;
        });
    }

    public function balances()
    {
        return $this->morphMany(Balance::class, 'holder');
    }

    public function lockedBalances()
    {
        return $this->morphMany(LockedBalance::class, 'holder');
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
        DB::transaction(function () use ($amount, $currency) {
            $balance = $this->balances()->where('currency', $currency->value)->lockForUpdate()
                ->firstOrFail();
            if ($balance->amount < $amount) {
                throw new Exception('Insufficient balance');
            }
            $balance->amount -= $amount;
            $balance->save();
        });
    }

    public function deposit(int $amount, Currency $currency)
    {
        DB::transaction(function () use ($amount, $currency) {
            $balance = $this->balances()->lockForUpdate()
                ->firstOrCreate(
                    ['currency' => $currency->value],
                    ['amount' => 0]
                );
            $balance->amount += $amount;
            $balance->save();
        });
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

    public function checkBalance(Currency $currency, $unlocked = false)
    {
        if ($currency == Currency::BTC) {
            if ($unlocked) {
                return DB::transaction(function () {
                    $balance = $this->balances()->where('currency', Currency::BTC)->lockForUpdate()->first();
                    if (! $balance) {
                        return 0;
                    }
                    $lockedBalanceAmountSats = $this->getLockedMSats();

                    return $balance->amount - $lockedBalanceAmountSats;
                });
            } else {
                $balance = $this->balances()->where('currency', Currency::BTC)->lockForUpdate()->first();

                return $balance ? $balance->amount : 0;
            }
        } else {
            throw new Exception('Unsupported currency');
        }
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
