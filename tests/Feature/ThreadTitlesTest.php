<?php

use App\Models\Message;
use App\Models\Thread;

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

### 2. Users
- **id**: bigint (auto-increment)
- **name**: string (nullable)
- **username**: string (unique, nullable)
- **email**: string (unique)
- **email_verified_at**: timestamp (nullable)
- **password**: string (nullable)
- **rememberToken**: string
- **profile_photo_path**: string (nullable, 2048 characters)
- **created_at**: timestamp
- **updated_at**: timestamp

### 3. Telescope Entries
- **sequence**: bigint (auto-increment)
- **uuid**: uuid
- **batch_id**: uuid
- **family_hash**: string (nullable)
- **should_display_on_index**: boolean
- **type**: string (20 characters)
- **content**: longText
- **created_at**: dateTime (nullable)

### 4. Jobs
- **id**: bigint (auto-increment)
- **queue**: string
- **payload**: longText
- **attempts**: unsignedTinyInteger
- **reserved_at**: unsignedInteger (nullable)
- **available_at**: unsignedInteger
- **created_at**: unsignedInteger

### 5. Subscription Items
- **id**: bigint (auto-increment)
- **subscription_id**: bigint (foreign key)
- **stripe_id**: string (unique)
- **stripe_product**: string
- **stripe_price**: string
- **quantity**: integer (nullable)
- **created_at**: timestamp
- **updated_at**: timestamp

### 6. Subscriptions
- **id**: bigint (auto-increment)
- **user_id**: bigint (foreign key)
- **type**: string
- **stripe_id**: string (unique)
- **stripe_status**: string
- **stripe_price**: string (nullable)
- **quantity**: integer (nullable)
- **trial_ends_at**: timestamp (nullable)
- **ends_at**: timestamp (nullable)
- **created_at**: timestamp
- **updated_at**: timestamp

### 7. Social Accounts
- **id**: bigint (auto-increment)
- **user_id**: bigint (foreign key)
- **provider_id**: string
- **provider_name**: string
- **provider_data**: json (nullable)
- **created_at**: timestamp
- **updated_at**: timestamp

These models and their fields are defined in the Eloquent model files and the corresponding migration files in the Laravel application.']);

    $this->artisan('threads:title')
        ->expectsOutput('Generating thread titles...')
        ->assertExitCode(0);

    dump($thread->refresh()->title);
})->skip();
