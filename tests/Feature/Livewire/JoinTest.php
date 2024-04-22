<?php

use App\Livewire\Auth\Join;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Join::class)
        ->assertStatus(200)
        ->assertSeeHtml('>Join OpenAgents</h2>')
        ->assertSee('Continue with X')
        ->assertSee('Continue with Nostr')
        ->assertSee('By continuing you agree to our')
        ->assertSee('Terms of Service')
        ->assertSee('Privacy Policy');
});
