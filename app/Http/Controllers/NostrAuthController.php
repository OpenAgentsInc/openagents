<?php

namespace App\Http\Controllers;

use App\Models\User;
use Exception;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use swentel\nostr\Event\Event;
use swentel\nostr\Key\Key;

class NostrAuthController extends Controller
{
    private function verifyEvent($json)
    {
        $event = new Event();
        $isValid = $event->verify($json);

        return $isValid;
    }

    public function testing()
    {
        $text = '
{
  "id": "62fa167369a603b1181a49ecf2e20e7189833417c3fb49666c5644901da27bcc",
  "pubkey": "84fdf029f065438702b011c2002b489fd00aaea69b18efeae8261c44826a8886",
  "created_at": 1689033061,
  "kind": 1,
  "tags": [],
  "content": "This event was created at https://nostrtool.com/ with a throwaway key.",
  "sig": "a67e8d286605e3d7dfd3e0bd1642f85a25bb0cd70ec2ed941349ac879f617868a3ffa2a9040bb43c024594a79e4878429a990298c51ae4d6d20533589f4a04df"
}';

        $event = json_decode($text);

        var_dump($this->verifyEvent(json_encode($event)));

        // Mangle last half-byte of the signature on purpose
        $event->sig[127] = 'e';

        var_dump($this->verifyEvent(json_encode($event)));
    }

    public function create(Request $request): RedirectResponse
    {
        // Get event from input and decode
        $eventBase64 = $request->input('event');
        $eventJson = base64_decode($eventBase64);
        if (! $eventJson) {
            return redirect('#error')->with('error', 'Invalid event data');
        }
        $event = json_decode($eventJson);

        // Check if it's a good authentication event
        if (! (is_object($event) && (
            $event->kind == 27235 &&
            $event->tags == [['u', 'https://openagents.com/login/nostr'], ['method', 'POST']] &&
            $event->content == ''
        ))) {
            return redirect('#error')->with('error', 'Invalid event format');
        }

        // Check if the event is within a 60-second range of the current time
        if (! (abs(time() - $event->created_at) <= 60)) {
            return redirect('#error')->with('error', 'Event timestamp is out of range');
        }

        // Verify the signature using the secp256k1_nostr extension
        $isValid = $this->verifyEvent($eventJson);
        if (! $isValid) {
            return redirect('#error')->with('error', 'Invalid event signature');
        }

        // Find
        try {
            // find user with auth_provider == nostr and external_id == pubkey
            $user = User::where('auth_provider', 'nostr')->where('external_id', $event->pubkey)->first();
        } catch (Exception $e) {
            return redirect('#error')->with('error', 'Database error: '.$e->getMessage());
        }

        $key = new Key();
        $bech32_public = $key->convertPublicKeyToBech32($event->pubkey);
        // If user not found, create a new user
        if (! $user) {
            $user = User::create([
                'external_id' => $event->pubkey,
                'profile_photo_path' => '/images/nostrich.jpeg',
                'username' => $bech32_public,
                'auth_provider' => 'nostr',
                'name' => substr($event->pubkey, 0, 8),
                'email' => substr($event->pubkey, 0, 8).'@notanemail.com',
            ]);
        }

        // update profile data
        $user->profile_photo_path = '/images/nostrich.jpeg';
        $user->save();

        // Log in this user
        auth()->login($user, true);

        return redirect('/')->with('success', 'Logged in successfully');
    }

    public function client()
    {
        return view('login-nostr');
    }
}
