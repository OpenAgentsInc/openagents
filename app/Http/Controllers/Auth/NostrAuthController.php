<?php

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Models\User;
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
        if (!$eventJson) return redirect('#error');
        $event = json_decode($eventJson);

        // Check if its a good authentication event
        if (!(
            $event->kind == 27235 &&
            $event->tags == [["u","https://openagents.com/login/nostr"],["method","POST"]] &&
            $event->content == ""
        )) return redirect("#error");

        // Check if event is in 60 seconds range of current time
        if (!(abs(time() - $event->created_at) <= 60)) return redirect("#error");

        // Verify the hash, pubkey and signature
        $isValid = (new Event)->verify($eventJson);
        if (!$isValid) return redirect('#error');

        // use existing user or create new with pubkey
        $user = User::updateOrCreate(
            ['nostr_pubkey' => $event->pubkey],
            [
                'name' => $event->pubkey,
            ]
        );

        // Log in this user
        auth()->login($user, true);

        // To dashboard
        return redirect("/dashboard");
    }

    public function client()
    {
        return view('auth/login-nostr');
    }

}

