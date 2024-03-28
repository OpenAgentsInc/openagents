<?php

use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {
        $browser->visit('/')
            ->assertSee('OpenAgents')
            ->assertSee('How can we help you today?')
            ->typeSlowly('@first-message-input', 'Your message here', 50)
            ->script("document.getElementById('send-message').click();");

        $browser->waitForText('OpenAgents')
            ->pause(5000);
    });
});
