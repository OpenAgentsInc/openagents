<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\Balance;
use App\Models\Payment;
use App\Models\PaymentDestination;
use App\Models\PaymentSource;
use App\Models\User;
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

        // Generate some payments
        Payment::factory()->count(20)->create()->each(function ($payment) {
            // Create random source for each payment
            $paymentSource = PaymentSource::factory()->make([
                'payment_id' => $payment->id,
                'source_type' => User::class,
                'source_id' => User::factory()->create()->id,
            ]);
            $payment->sources()->save($paymentSource);

            // Create random destination for each payment
            $paymentDestination = PaymentDestination::factory()->make([
                'payment_id' => $payment->id,
                'destination_type' => Agent::class,
                'destination_id' => Agent::factory()->create()->id,
            ]);
            $payment->destinations()->save($paymentDestination);
        });
    }
}
