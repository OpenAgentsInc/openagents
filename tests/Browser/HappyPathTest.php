<?php

use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {

        // Homepage loads and allows chat
        $browser->visit('/')
            ->assertSee('OpenAgents')
            ->assertSee('How can we help you today?')
            ->typeSlowly('#message-input', 'Who are you?', 50)
            ->script("document.getElementById('send-message').click();");

        // Lets see if we get an answer
        $browser->waitForText('artificial intelligence');
    });
});
