<?php

namespace App\Http\Controllers;

use App\Models\NostrAccount;
use App\Models\User;
use Exception;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Mdanter\Ecc\Crypto\Signature\SchnorrSignature;
use swentel\nostr\Event\Event;

class NostrAuthController extends Controller
{
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

        var_dump(secp256k1_nostr_verify($event->pubkey, $event->id, $event->sig));

        // Mangle last half-byte of the signature on purpose
        $event->sig[127] = 'e';

        var_dump(secp256k1_nostr_verify($event->pubkey, $event->id, $event->sig));
    }

    public function testing2()
    {
        //        $adapter = EccFactory::getAdapter();
        //        $generator = EccFactory::getNistCurves()->generator384();
        //        $private = $generator->createPrivateKey();
        //
        //        $derSerializer = new DerPrivateKeySerializer($adapter);
        //        $der = $derSerializer->serialize($private);
        //        echo sprintf("DER encoding:\n%s\n\n", base64_encode($der));
        //
        //        $pemSerializer = new PemPrivateKeySerializer($derSerializer);
        //        $pem = $pemSerializer->serialize($private);
        //        echo sprintf("PEM encoding:\n%s\n\n", $pem);

        $pubkey = '07adfda9c5adc80881bb2a5220f6e3181e0c043b90fa115c4f183464022968e6';
        $signature = '49352dbe20322a9cc40433537a147805e2541846c006a3e06d9f90faadb89c83ee6da24807fb9eddc6ed9a1d3c15cd5438df07ec6149d6bf48fe1312c9593567';
        $message = 'd677b5efa1484e3461884d6ba01e78b7ced36ccfc4b5b873c0b4142ea574938f';

        var_dump((new SchnorrSignature())->verify($pubkey, $signature, $message));

        return 'ok';
    }

    public function create2(Request $request): RedirectResponse
    {
        // Get event from input and decode
        $eventBase64 = $request->input('event');
        $eventJson = base64_decode($eventBase64);
        if (! $eventJson) {
            dd('fucking what');

            return redirect('#error');
        }
        $event = json_decode($eventJson);

        // Check if its a good authentication event
        if (! (
            $event->kind == 27235 &&
            $event->tags == [['u', 'https://openagents.com/login/nostr'], ['method', 'POST']] &&
            $event->content == ''
        )) {
            dd('what dis');

            return redirect('#error');
        }

        // Check if event is in 60 seconds range of current time
        if (! (abs(time() - $event->created_at) <= 60)) {
            dd('huh no bad');

            return redirect('#error');
        }

        // Verify the hash, pubkey and signature
        $isValid = (new Event)->verify($eventJson);
        if (! $isValid) {
            dd('not valid, redirecting with error');

            return redirect('#error');
        }

        dd('here');

        // Find a user with this pubkey
        try {
            $user = User::whereHas('nostrAccount', function ($query) use ($event) {
                $query->where('pubkey', $event->pubkey);
            })->first();
        } catch (Exception $e) {
            dd($e->getMessage());
        }

        // If user not found, create a new user
        if (! $user) {
            $user = User::create([
                'name' => substr($event->pubkey, 0, 8),
                'email' => '',
            ]);

            NostrAccount::create([
                'user_id' => $user->id,
                'pubkey' => $event->pubkey,
            ]);
        }

        // Log in this user
        auth()->login($user, true);

        return redirect('/');
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
        if (! (
            $event->kind == 27235 &&
            $event->tags == [['u', 'https://openagents.com/login/nostr'], ['method', 'POST']] &&
            $event->content == ''
        )) {
            return redirect('#error')->with('error', 'Invalid event format');
        }

        // Check if the event is within a 60-second range of the current time
        if (! (abs(time() - $event->created_at) <= 60)) {
            return redirect('#error')->with('error', 'Event timestamp is out of range');
        }

        // Verify the signature using the secp256k1_nostr extension
        $isValid = secp256k1_nostr_verify($event->pubkey, $event->id, $event->sig);
        if (! $isValid) {
            return redirect('#error')->with('error', 'Invalid event signature');
        }

        // Find a user with this pubkey
        try {
            $user = User::whereHas('nostrAccount', function ($query) use ($event) {
                $query->where('pubkey', $event->pubkey);
            })->first();
        } catch (Exception $e) {
            return redirect('#error')->with('error', 'Database error: '.$e->getMessage());
        }

        // If user not found, create a new user
        if (! $user) {
            $user = User::create([
                'name' => substr($event->pubkey, 0, 8),
                'email' => '',
            ]);

            NostrAccount::create([
                'user_id' => $user->id,
                'pubkey' => $event->pubkey,
            ]);
        }

        // Log in this user
        auth()->login($user, true);

        return redirect('/')->with('success', 'Logged in successfully');
    }

    public function client()
    {
        return view('login-nostr');
    }
}
