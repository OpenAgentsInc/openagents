<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\DB;

class SendUsageReport extends Command
{
    protected $signature = 'report:usage';

    protected $description = 'Report the number of user signups in the last 24 hours';

    public function handle()
    {
        $count = DB::table('users')
            ->where('created_at', '>=', now()->subDay())
            ->count();

        $message = "Number of user signups in the last 24 hours: $count";

        $this->info($message);

        //        Log::channel('slack')->info($message);
    }
}
