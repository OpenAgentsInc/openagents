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
            'is_default' => false,
            'custom_instructions' => null,
            'context' => null,
            'settings' => null,
            'status' => 'active',
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
    public function forTeam($team = null)
    {
        return $this->state(function (array $attributes) use ($team) {
            return [
                'user_id' => null,
                'team_id' => $team ?? Team::factory(),
            ];
        });
    }

    /**
     * Indicate that the project is archived.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function archived()
    {
        return $this->state(function (array $attributes) {
            return [
                'status' => 'archived',
            ];
        });
    }

    /**
     * Indicate that the project has custom instructions.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function withInstructions($instructions = null)
    {
        return $this->state(function (array $attributes) use ($instructions) {
            return [
                'custom_instructions' => $instructions ?? $this->faker->paragraph,
            ];
        });
    }

    /**
     * Indicate that the project has context.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function withContext($context = null)
    {
        return $this->state(function (array $attributes) use ($context) {
            return [
                'context' => $context ?? $this->faker->paragraph,
            ];
        });
    }

    /**
     * Indicate that the project has settings.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function withSettings(array $settings = null)
    {
        return $this->state(function (array $attributes) use ($settings) {
            return [
                'settings' => $settings ?? [
                    'tone' => $this->faker->randomElement(['formal', 'casual', 'technical']),
                    'language' => $this->faker->languageCode(),
                    'role' => $this->faker->jobTitle(),
                ],
            ];
        });
    }
}