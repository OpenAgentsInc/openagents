<?php

use Laravel\Dusk\Browser;

test('Happy Path', function () {
    $this->browse(function (Browser $browser) {
        $browser->visit('/')

            // I see OpenAgents
            ->waitForText('OpenAgents')
            ->assertSee('OpenAgents')

            // I see login buttons
            ->assertSee('Log in')
            ->assertSee('Sign up')

            // I can click sign up
            ->clickLink('Sign up')

            // The signup page loads
            ->waitForText('Sign up for OpenAgents')
            ->assertPathIs('/register')

            // I see a form and can fill it out (name, email, password, confirm)
            ->type('name', 'John Doe')
            ->type('email', 'joe@jammersmith.com')
            ->type('password', 'password')
            ->type('password_confirmation', 'password')

            // I can submit the form
            ->click('button[type="submit"]')

            // I see the form with prompt
            ->waitForText('How can we help?')
            ->assertSee('How can we help?')

            ->screenshot('done');
    });
});
