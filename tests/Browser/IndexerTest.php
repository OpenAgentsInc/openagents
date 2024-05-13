<?php

use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;
use Laravel\Dusk\Browser;

test('can interact with paid indexer agent', function () {
    $this->browse(function (Browser $browser) {

        $currentAgentCount = Agent::count();

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
            ->waitFor('@model-dropdown')
            ->click('@model-dropdown')
            ->waitFor('@create-agent')
            ->click('@create-agent')
            ->waitForRoute('agents.create')
            ->assertPathIs('/build')
            ->waitFor('@name')

            // Can fill out the form to create an agent
            ->type('@name', 'OA Codebase Indexer')
            ->type('@description', 'Queries an index of the OpenAgents.com codebase on GitHub.')
            ->type('@instructions', 'You consult the index and provide summaries of codebase sections.')
            ->click('@create-agent-button')
            ->pause(500);

        // Assert we have one more agent in the database
        $this->assertEquals($currentAgentCount + 1, Agent::count());

        // Get the ID of the most recent thread
        $threadId = Thread::latest()->first()->id;

        // User is redirected to chat with the agent
        $browser
            ->waitForRoute('chat.id', ['id' => $threadId])
            ->assertSee('OA Codebase Indexer')
            ->waitFor('@message-input')
            ->type('@message-input', 'Have you indexed this repo? https://github.com/OpenAgentsInc/openagents')
            ->click('@send-message-button')
            ->pause(90000)
            ->screenshot('indexer-test2');
    });
});
