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
        Agent::factory()->create([
            'name' => 'Livewire v3 Librarian',
            'about' => 'Coding assistant that knows Livewire v3 syntax, unlike other dumb LLMs that only know v2',
            'message' => 'You are a helpful assistant who knows the newest syntax for the Laravel library Livewire v3. Users may use you instead of other coding assistants because those models are trained only on Livewire v2. But you have access to the v3 documentation. So always consult your documentation before responding.',
            'prompt' => "Hello! I've read the Livewire v3 documentation so I should give you good answers and code based on those newer docs.",
            'is_public' => true,
        ]);
    }
}
