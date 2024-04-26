<?php

use App\Models\User;
use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {

        // Homepage loads and shows title and Join button
        $browser
            ->move(100, 100)
            ->resize(1500, 1000)
            ->visit('/')
            ->assertSee('OpenAgents')
            ->assertSee('Who would you')
            ->assertSee('Join');

        $browser->loginAs(User::factory()->create());

        $browser->visit('/')
            ->assertSee('Create an agent')
            ->click('@create-agent')
            ->assertPathIs('/create');

        // Mock a login

        //            ->typeSlowly('#message-input', 'Who are you?', 50)
        //            ->script("document.getElementById('send-message').click();");

        // Lets see if we get an answer
        //        $browser->waitForText('artificial intelligence');
    });
});
