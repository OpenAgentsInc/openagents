<?php

use App\Livewire\Changelog;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Changelog::class)
        ->assertStatus(200)
        ->assertSee('Changelog')
        ->assertSeeHtml('See our <a href="https://github.com/OpenAgentsInc/openagents/commits/main/" target="_blank">GitHub');
});
