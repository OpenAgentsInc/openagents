<?php

use App\Models\User;
use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {

        // Homepage loads and shows title and Join button
        $browser
            ->move(150, 50)
            ->resize(1400, 1000)
            ->visit('/')
            ->assertSee('OpenAgents')
            ->assertSee('How can we help you')
            ->assertSee('Join');

        // Mock a login
        $browser->loginAs(User::factory()->create());

        // Can click on Create Agent and go to new create page
        $browser->visit('/')
            ->assertSee('Create an agent')
            ->click('@create-agent')
            ->waitForText('New Agent')
            ->assertPathIs('/create')
            ->pause(1500)

            ->typeSlowly('name', 'Yo Momma')
            ->pause(50)
            ->typeSlowly('description', 'does cool shit')
            ->pause(50)
            ->typeSlowly('instructions', 'your mom');
    });
});
