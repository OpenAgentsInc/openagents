<?php

use App\Livewire\Settings;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    $this->actingAs(User::factory()->create());

    Livewire::test(Settings::class)
        ->assertStatus(200)
        ->assertSeeHtml('>Settings</h3>')
        ->assertSeeHtml('>Default model for new chats</div>')
        ->assertSeeHtml('>Autoscroll to bottom in chats</div>')
        ->assertSeeHtml('>Lightning Address</div>');
});

it('disables autoscroll when clicked', function () {
    $this->actingAs(User::factory()->create());

    Livewire::test(Settings::class)
        ->assertSeeInOrder(['Autoscroll', 'DISABLED'])
        ->call('toggleAutoscroll')
        ->assertSeeInOrder(['Autoscroll', 'ENABLED'])
        ->call('toggleAutoscroll')
        ->assertSeeInOrder(['Autoscroll', 'DISABLED']);
});
