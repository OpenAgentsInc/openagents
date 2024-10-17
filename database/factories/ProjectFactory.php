<?php

namespace Database\Factories;

use App\Models\Team;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\Project>
 */
class ProjectFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => $this->faker->words(3, true),
            'description' => $this->faker->sentence,
            'user_id' => null,
            'team_id' => null,
        ];
    }

    /**
     * Indicate that the project belongs to a user.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function forUser()
    {
        return $this->state(function (array $attributes) {
            return [
                'user_id' => User::factory(),
                'team_id' => null,
            ];
        });
    }

    /**
     * Indicate that the project belongs to a team.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function forTeam()
    {
        return $this->state(function (array $attributes) {
            return [
                'user_id' => null,
                'team_id' => Team::factory(),
            ];
        });
    }
}