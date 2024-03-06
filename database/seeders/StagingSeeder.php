<?php

namespace Database\Seeders;

use Illuminate\Database\Seeder;

class StagingSeeder extends Seeder
{
    public function run(): void
    {
        // Run the AdminUserSeeder
        $this->call(AdminUserSeeder::class);

        // Then run the AgentBuilderSeeder
        $this->call(AgentBuilderSeeder::class);
    }
}
