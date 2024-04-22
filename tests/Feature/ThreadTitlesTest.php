<?php

use App\Models\Message;
use App\Models\Thread;
use GuzzleHttp\Client;

test('can generate thread titles', function () {
    $thread = Thread::factory()->create(['title' => 'New chat']);
    Message::factory()->create(['thread_id' => $thread->id, 'body' => 'List the main data models used and their fields and column types.']);
    Message::factory()->create(['thread_id' => $thread->id, 'body' => 'Here are the main data models used in the `openagentsinc/openagents` repository, along with their fields and column types:

### 1. Messages
- **id**: bigint (auto-increment)
- **thread_id**: bigint (foreign key)
- **user_id**: bigint (foreign key, nullable)
- **body**: longText
- **model**: text (nullable)
- **session_id**: string (nullable)
- **input_tokens**: integer (nullable)
- **output_tokens**: integer (nullable)
- **created_at**: timestamp
- **updated_at**: timestamp
...

These models and their fields are defined in the Eloquent model files and the corresponding migration files in the Laravel application.']);

    $mockResponse = [
        'text' => 'Data Models Overview But Lets Pretend It Is Longer Than 7 Words',
    ];
    $httpClient = mockGuzzleClient($mockResponse);
    $this->app->instance(Client::class, $httpClient);

    $this->artisan('threads:title')
        ->expectsOutput('Generating thread titles...')
        ->assertExitCode(0);

    $this->assertEquals(
        'Data Models Overview But Lets Pretend It',
        $thread->refresh()->title
    );
});
