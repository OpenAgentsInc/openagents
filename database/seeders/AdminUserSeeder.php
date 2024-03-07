<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;

class AdminUserSeeder extends Seeder
{
    public function run(): void
    {
        // Create user #1
        User::factory()->create([
            'id' => 1,
            'name' => 'Admin',
            'email' => 'blah@blah.com',
        ]);
    }
}
