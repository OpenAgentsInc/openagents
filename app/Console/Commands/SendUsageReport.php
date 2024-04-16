<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;

class SendUsageReport extends Command
{
    protected $signature = 'report:usage';

    protected $description = 'Report the number of user signups, active subscriptions, and total users';

    public function handle()
    {
        $userCount = DB::table('users')
            ->where('created_at', '>=', now()->subDay())
            ->count();

        $activeSubscriptions = DB::table('subscriptions')
            ->where('stripe_status', 'active')
            ->count();

        $totalUsers = DB::table('users')->count();

        $mrr = $activeSubscriptions * 10;

        $message = "Total users: $totalUsers \n";
        $message .= "User signups in the last 24 hours: $userCount\n";
        $message .= "Pro subscriptions: $activeSubscriptions ($".$mrr." MRR)\n";

        $this->info($message);

        Log::channel('slack')->info($message);
    }
}
