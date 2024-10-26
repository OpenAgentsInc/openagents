<?php

namespace App\Traits;

use App\Models\CreditTransaction;
use Illuminate\Database\Eloquent\Relations\MorphMany;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Auth;

trait UsesCredits
{
    public function creditTransactions(): HasMany
    {
        return $this->hasMany(CreditTransaction::class, $this instanceof \App\Models\Team ? 'team_id' : 'user_id');
    }

    public function updateCredits(float $amount, string $type, ?string $description = null, $creditable = null): float
    {
        Log::info('Updating credits', [
            'model' => get_class($this),
            'id' => $this->id,
            'current_credits' => $this->credits,
            'amount' => $amount,
            'type' => $type,
            'description' => $description,
            'creditable' => $creditable ? get_class($creditable) . ':' . $creditable->id : null
        ]);

        return DB::transaction(function () use ($amount, $type, $description, $creditable) {
            $newBalance = $this->credits + $amount;

            $transactionData = [
                'amount' => $amount,
                'type' => $type,
                'description' => $description,
                'user_id' => Auth::id(),
            ];

            if ($this instanceof \App\Models\Team) {
                $transactionData['team_id'] = $this->id;
            } elseif ($this instanceof \App\Models\User) {
                $transactionData['user_id'] = $this->id;
            }

            if ($creditable) {
                $transactionData['creditable_type'] = get_class($creditable);
                $transactionData['creditable_id'] = $creditable->id;
            }

            $transaction = $this->creditTransactions()->create($transactionData);

            Log::info('Credit transaction created', [
                'model' => get_class($this),
                'id' => $this->id,
                'transaction_id' => $transaction->id,
                'transaction_data' => $transaction->toArray()
            ]);

            $this->credits = $newBalance;
            $this->save();

            Log::info('Credits updated', [
                'model' => get_class($this),
                'id' => $this->id,
                'new_credits' => $this->credits
            ]);

            return $this->credits;
        });
    }

    public function setCreditsAttribute($value): void
    {
        $this->attributes['credits'] = round($value, 4);
    }

    public function getCreditsAttribute($value): float
    {
        return round($value, 4);
    }
}