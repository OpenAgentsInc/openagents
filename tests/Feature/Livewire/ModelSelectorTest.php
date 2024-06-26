<?php

use App\Livewire\ModelSelector;
use App\Models\Thread;
use Livewire\Livewire;

it('renders successfully', function () {

    $thread = Thread::factory()->create([]);
    Livewire::test(ModelSelector::class, ['thread' => $thread])
        ->assertStatus(200)
        ->assertSeeInOrder([
            'Llama 3 8B', 'Join',
            'Llama 3 70B', 'Pro',
            'Mistral Small',
            'Mistral Medium', 'Join',
            'Mistral Large', 'Pro',
            'Codestral', 'Pro',
            'GPT-3.5 Turbo 16K', 'Join',
            'GPT-4 Turbo Preview', 'Join',
            'GPT-4 Turbo 2024-04-09', 'Join',
            'GPT-4', 'Pro',
            'Claude Haiku',
            'Claude Sonnet', 'Join',
            'Claude Opus', 'Pro',
            'Sonar Small Online', 'Join',
            'Sonar Medium Online', 'Pro',
            'Command-R', 'Join',
            'Command-R+', 'Join',
            'Satoshi 7B',
            'Greptile: OA Codebase', 'Join',
        ]);
})->skip();
