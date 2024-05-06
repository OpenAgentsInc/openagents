<?php

namespace Database\Seeders;

use App\Models\Agent;
use Illuminate\Database\Seeder;

class AgentSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        Agent::create([
            'name' => 'OA Repo Analyzer',
            'about' => 'Queries an index of the OpenAgents.com codebase on GitHub.',
            'prompt' => 'You consult the index and provide summaries of codebase sections.',
            'image' => null,
            'is_public' => true,
        ]);
    }
}
