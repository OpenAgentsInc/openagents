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
        $multiplier = 5000;

        /**
         * SELECT users.*
         * FROM users
         * JOIN agents ON users.id = agents.user_id
         * WHERE users.lightning_address IS NOT NULL;
         * */

        // Get all users who have agents and have a lightning address
        $users = User::join('agents', 'users.id', '=', 'agents.user_id')
            ->whereNotNull('users.lightning_address')
            ->distinct()
            ->get();

        $this->info('Found '.$users->count().' users with agents and lightning addresses.');

        // Loop through each user
        foreach ($users as $user) {
            $this->info('Paying '.$user->name.' to Lightning Address '.$user->lightning_address.'...');

            // Get the thread_count and unique_users_count of all agents of this user
            $thread_count = $user->agents->sum('thread_count');
            $unique_users_count = $user->agents->sum('unique_users_count');

            // Calculate the total amount to pay
            $total_score = $thread_count + $unique_users_count * 3;
            $sats_payout = $total_score * $multiplier;
            $this->info('Total score: '.$total_score.' | Payout: '.$sats_payout.' sats');
        }
    }
}
