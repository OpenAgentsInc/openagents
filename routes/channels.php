<?php

use App\Models\Agent;
use App\Models\Run;
use Illuminate\Support\Facades\Broadcast;

/*
|--------------------------------------------------------------------------
| Broadcast Channels
|--------------------------------------------------------------------------
|
| Here you may register all of the event broadcasting channels that your
| application supports. The given channel authorization callbacks are
| used to check if an authenticated user can listen to the channel.
|
*/

// Broadcast::channel('Agent.{id}', function ($user, Agent $agent) {
//     return (int) $agent->user->id === (int) $user->id;
// });
