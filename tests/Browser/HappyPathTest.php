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
            ->assertSee('Who would you')
            ->assertSee('Join');

        // Mock a login
        $browser->loginAs(User::factory()->create());

        // Can click on Create Agent and go to new create page
        $browser->visit('/')
            ->assertSee('Create an agent')
            ->click('@create-agent')
            ->waitForText('Create agent')
            ->assertPathIs('/create')
            ->pause(3000);
    });
});
