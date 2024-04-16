<?php

namespace App\Http\Controllers;

use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use swentel\nostr\Event\Event;

class NostrAuthController extends Controller
{
    public function create(Request $request): RedirectResponse
    {
        // Get event from input and decode
        $eventBase64 = $request->input('event');
        $eventJson = base64_decode($eventBase64);
        if (! $eventJson) {
            return redirect('#error');
        }
        $event = json_decode($eventJson);

        // Check if its a good authentication event
        if (! (
            $event->kind == 27235 &&
            $event->tags == [['u', 'https://openagents.com/login/nostr'], ['method', 'POST']] &&
            $event->content == ''
        )) {
            return redirect('#error');
        }

        // Check if event is in 60 seconds range of current time
        if (! (abs(time() - $event->created_at) <= 60)) {
            return redirect('#error');
        }

        // Verify the hash, pubkey and signature
        $isValid = (new Event)->verify($eventJson);
        if (! $isValid) {
            return redirect('#error');
        }

        // Find a user with this pubkey
        $user = NostrAccount::where('pubkey', $event->pubkey)->first();

        // If user not found, create a new user
        if (! $user) {
            $user = NostrAccount::create([
                'pubkey' => $event->pubkey,
                'name' => $event->pubkey,
            ]);
        }

        // Log in this user
        auth()->login($user, true);

        return redirect('/');
    }

    public function client()
    {
        return view('login-nostr');
    }
}
