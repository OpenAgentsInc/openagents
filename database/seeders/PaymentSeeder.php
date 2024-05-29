<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Balance;
use App\Models\Payment;
use App\Models\PaymentDestination;
use App\Models\PaymentSource;
use App\Models\User;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Seeder;

// Replace with actual holder/payer models
// Replace with actual source/destination models

class PaymentSeeder extends Seeder
{
    /**
     * Run the database seeds.
     *
     * @return void
     */
    public function run()
    {
        // Generate balances for some users/accounts (holders)
        $users = User::factory()->count(10)->create();
        foreach ($users as $user) {
            Balance::factory()->create([
                'holder_type' => User::class,
                'holder_id' => $user->id,
            ]);
        }

        // Generate balances for agents as well
        $agents = Agent::factory()->count(10)->create();
        foreach ($agents as $agent) {
            Balance::factory()->create([
                'holder_type' => Agent::class,
                'holder_id' => $agent->id,
            ]);
        }

        // Generate some payments
        Payment::factory()->count(20)->create()->each(function ($payment) use ($users, $agents) {
            // Randomly pick a source, either User or Agent
            $sourceModel = $this->getRandomModel($users, $agents);
            $paymentSource = PaymentSource::factory()->make([
                'payment_id' => $payment->id,
                'source_type' => get_class($sourceModel),
                'source_id' => $sourceModel->id,
            ]);
            $payment->sources()->save($paymentSource);

            // Randomly pick a destination, either User or Agent
            $destinationModel = $this->getRandomModel($users, $agents);
            $paymentDestination = PaymentDestination::factory()->make([
                'payment_id' => $payment->id,
                'destination_type' => get_class($destinationModel),
                'destination_id' => $destinationModel->id,
            ]);
            $payment->destinations()->save($paymentDestination);
        });
    }

    /**
     * Get a random model instance from either Users or Agents collections.
     *
     * @param  Collection  $users
     * @param  Collection  $agents
     * @return Model
     */
    protected function getRandomModel($users, $agents)
    {
        return [$users->random(), $agents->random()][random_int(0, 1)];
    }
}
