<?php

namespace App\Traits;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
use Exception;
use Illuminate\Support\Facades\DB;

trait Payable
{
    public function payAgent(Agent $agent, int $amount, Currency $currency)
    {
        DB::transaction(function () use ($agent, $amount, $currency) {
            $this->withdraw($amount, $currency);
            $agent->deposit($amount, $currency);
        });
    }

    public function withdraw(int $amount, Currency $currency)
    {
        $balance = $this->balances()->where('currency', $currency)->firstOrFail();
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
            ['currency' => $currency],
            ['amount' => 0]
        );
        $balance->amount += $amount;
        $balance->save();
    }

    public function checkBalance(Currency $currency)
    {
        $balance = $this->balances()->where('currency', $currency)->first();

        return $balance ? $balance->amount : 0;
    }

    public function getBalanceAttribute()
    {
        return $this->balances()->get();
    }
}
