<?php

namespace App\Traits;

use Carbon\Carbon;
use App\Models\CreditTransaction;
use App\Models\Message;
use Illuminate\Support\Facades\DB;

trait HasAgentStats
{
    public function getSpentCreditsLast24Hours()
    {
        $twentyFourHoursAgo = Carbon::now()->subDay();

        return CreditTransaction::where('team_id', $this->id)
            ->where('type', 'usage')
            ->where('created_at', '>=', $twentyFourHoursAgo)
            ->sum(DB::raw('ABS(amount)'));
    }

    public function getTotalAgentActions()
    {
        return Message::where('team_id', $this->id)
            ->whereNull('user_id')
            ->count();
    }

    public function getAgentActionsLast24Hours()
    {
        $twentyFourHoursAgo = Carbon::now()->subDay();

        return Message::where('team_id', $this->id)
            ->whereNull('user_id')
            ->where('created_at', '>=', $twentyFourHoursAgo)
            ->count();
    }

    // public function updateCredits($amount)
    // {
    //     $this->credits += $amount;
    //     $this->save();

    //     CreditTransaction::create([
    //         'team_id' => $this->id,
    //         'amount' => $amount,
    //         'type' => $amount > 0 ? 'credit' : 'usage',
    //         'description' => $amount > 0 ? 'Credit added' : 'Credit used',
    //     ]);

    //     return $this->credits;
    // }
}
