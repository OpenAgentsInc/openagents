<?php

namespace Database\Seeders;

use App\Models\Codebase;
use Illuminate\Database\Seeder;

class CodebaseSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        Codebase::create([
            'repository' => 'openagentsinc/openagents',
            'remote' => 'github',
            'branch' => 'main',
            'private' => false,
            'status' => 'completed',
            'files_processed' => 1075,
            'num_files' => 1075,
            'sample_questions' => json_encode([]),
            'sha' => 'f15781e1bac3108f5a0104057a92cc0bb6512e5c',
        ]);
    }
}
