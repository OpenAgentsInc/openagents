<?php

use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {
        $browser->visit('/')
            ->assertSee('OpenAgents');
    });
});
