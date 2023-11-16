<?php

namespace Database\Factories;

use App\Models\Embedding;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;
use Illuminate\Support\Facades\Hash;
use Illuminate\Support\Str;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\File>
 */
class FileFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'user_id' => User::factory(),
            'path' => '/app/blah/yo.txt'
        ];
    }

    /**
     * Indicate that the File should have a number of Embeddings.
     *
     * @param  int  $count
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function withEmbeddings($count = 1)
    {
        return $this->afterCreating(function ($file) use ($count) {
            Embedding::factory()->count($count)->create([
                'file_id' => $file->id,
            ]);
        });
    }
}
