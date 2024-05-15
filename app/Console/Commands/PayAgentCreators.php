<?php

namespace App\Console\Commands;

use App\Models\User;
use App\Services\LocalLogger;
use App\Services\PrismService;
use Exception;
use Illuminate\Console\Command;

class PayAgentCreators extends Command
{
    protected $signature = 'payout';

    protected $description = 'Pays agent creators';

    protected $prismService;

    public function __construct(PrismService $prismService)
    {
        parent::__construct();
        $this->prismService = $prismService;
    }

    public function handle()
    {
        $this->info('Paying agent creators.');
        $totalPayout = 100000; // Total payout in sats

        // Get all users who have agents and a lightning address, excluding developers
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

        $this->payUsers($users, $minPayout, $remainingPayout);
    }

    public function payUsers($users, $minPayout, $remainingPayout)
    {
        foreach ($users as $user) {
            // Check if the user has an associated Prism user
            if (! $this->hasPrismUser($user)) {
                // Create a Prism user if one does not exist
                $this->createPrismUser($user);
            }

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

            // Proceed with the payment logic
            $this->payUser($user, $payout);
        }
    }

    protected function hasPrismUser(User $user)
    {
        // Check if the user has an associated Prism user
        return ! empty($user->prism_user_id);
    }

    /**
     * @throws Exception
     */
    protected function createPrismUser(User $user)
    {
        // Use the PrismService to create a Prism user
        $prismUser = $this->prismService->createUser([
            'lnAddress' => $user->lightning_address,
        ]);

        $logger = new LocalLogger();
        $logger->log('Created Prism user: '.json_encode($prismUser));

        if (isset($prismUser['id'])) {
            $user->prism_user_id = $prismUser['id'];
            $user->save();
        } else {
            // throw exception
            throw new Exception('Failed to create Prism user for '.$user->name);
        }
    }

    protected function payUser(User $user, $payout)
    {
        // Implement the payment logic using the Prism service
        $recipient = [
            'lnAddress' => $user->lightning_address,
            'amount' => $payout,
        ];

        $this->prismService->sendPayment($payout, [$recipient]);
    }
}
