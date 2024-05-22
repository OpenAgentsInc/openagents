<?php

use App\Livewire\ProWelcome;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(ProWelcome::class)
        ->assertStatus(200)
        ->assertSee('A message from the founder')
        ->assertSeeHtml('<iframe src="https://player.vimeo.com/video/932242103')
        ->assertSeeHtml('<a href="https://twitter.com/OpenAgentsInc"')
        ->assertSee('DM or tag');
});
