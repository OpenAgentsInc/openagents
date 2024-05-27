<?php

namespace App\Traits;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
use App\Models\Payment;
use App\Models\User;
use Exception;
use Illuminate\Support\Facades\DB;

trait Payable
{
    public function payments()
    {
        return $this->morphMany(Payment::class, 'payer');
    }

    public function payAgent(Agent $agent, int $amount, Currency $currency)
    {
        DB::transaction(function () use ($agent, $amount, $currency) {
            $this->withdraw($amount, $currency);
            $agent->deposit($amount, $currency);
            $this->recordPayment($amount, $currency);
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

    private function recordPayment(int $amount, Currency $currency)
    {
        Payment::create([
            'payer_type' => get_class($this),
            'payer_id' => $this->id,
            'currency' => $currency->value,
            'amount' => $amount,
        ]);
    }

    public function payUser(User $user, int $amount, Currency $currency)
    {
        DB::transaction(function () use ($user, $amount, $currency) {
            $this->withdraw($amount, $currency);
            $user->deposit($amount, $currency);
            $this->recordPayment($amount, $currency);
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
}
