<?php

namespace App\Console\Commands;

use App\Models\User;
use Illuminate\Console\Command;

class PayAgentCreators extends Command
{
    protected $signature = 'payout';

    protected $description = 'Pays agent creators';

    public function handle()
    {
        $this->info('Paying agent creators.');
        $totalPayout = 100000; // Total payout in sats

        // Get all users who have agents and have a lightning address, excluding developers
        $users = User::whereIn('id', function ($query) {
            $query->select('user_id')
                ->from('agents')
                ->whereNotNull('lightning_address')
                ->whereNotIn('lightning_address', ['atlantispleb@getalby.com', 'svemir@coinos.io'])
                ->groupBy('user_id');
        })->get();

        $this->info('Found '.$users->count().' users with agents and lightning addresses (excluding developers).');

        $minPayout = 10000; // Minimum payout for users who created an agent
        $remainingPayout = $totalPayout - ($users->count() * $minPayout); // Remaining payout after minimum payouts

        foreach ($users as $user) {
            $this->info('Paying '.$user->name.' to Lightning Address '.$user->lightning_address.'...');

            $threadCount = $user->agents->sum('thread_count');
            $uniqueUsersCount = $user->agents->sum('unique_users_count');
            $totalScore = $threadCount + $uniqueUsersCount * 3;

            $payout = $minPayout;

            if ($totalScore > 0 && $remainingPayout > 0) {
                $scorePercentage = $totalScore / $users->sum(function ($user) {
                    $threadCount = $user->agents->sum('thread_count');
                    $uniqueUsersCount = $user->agents->sum('unique_users_count');

                    return $threadCount + $uniqueUsersCount * 3;
                });

                $payout += $remainingPayout * $scorePercentage;
            }

            $this->info('Total score: '.$totalScore.' | Payout: '.$payout.' sats');
        }
    }
}
