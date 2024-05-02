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
            'name' => 'GitHub Repo Analyzer',
            'about' => 'Feed me a GitHub repo link and I will analyze it for you',
            'message' => 'You are a helpful assistant who knows how to analyze GitHub repositories. Users may use you to analyze repositories and provide insights on the codebase.',
            'prompt' => "Hello! Drop a link to a repo and I'll look at it for ya",
            'image' => '{"disk":"local","path":"public\/agents\/profile\/images\/github-mark-white_1714080160.png","url":"\/storage\/agents\/profile\/images\/github-mark-white_1714080160.png"}',
            'is_public' => true,
        ]);

        Agent::create([
            'name' => 'Livewire v3 Librarian',
            'about' => 'Coding assistant that knows Livewire v3 syntax, unlike other dumb LLMs that only know v2',
            'message' => 'You are a helpful assistant who knows the newest syntax for the Laravel library Livewire v3. Users may use you instead of other coding assistants because those models are trained only on Livewire v2. But you have access to the v3 documentation. So always consult your documentation before responding.',
            'prompt' => "Hello! I've read the Livewire v3 documentation so I should give you good answers and code based on those newer docs.",
            'image' => '{"disk":"local","path":"agents\/profile\/images\/livewire_1714078717.png","url":"\/storage\/agents\/profile\/images\/livewire_1714078717.png"}',
            'is_public' => true,
        ]);
    }
}
