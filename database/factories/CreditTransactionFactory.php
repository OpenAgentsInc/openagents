<?php

namespace Database\Factories;

use App\Models\CreditTransaction;
use App\Models\Team;
use Illuminate\Database\Eloquent\Factories\Factory;

class CreditTransactionFactory extends Factory
{
    protected $model = CreditTransaction::class;

    public function definition()
    {
        return [
            'team_id' => Team::factory(),
            'amount' => $this->faker->randomFloat(2, 10, 1000),
            'description' => $this->faker->sentence(),
            'type' => $this->faker->randomElement(['credit', 'debit']),
        ];
    }
}
