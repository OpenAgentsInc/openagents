<?php

namespace App\Http\Controllers;

class NostrController extends Controller
{

    public function plugincreate()
    {
        return view('nostr-plugin-create');
    }

    public function settings()
    {
        return view('nostr-settings');
    }

    public function plugin($pubkey, $title)
    {
        return view('nostr-plugin', ['pubkey' => $pubkey, 'title' => $title,]);
    }

    public function plugins()
    {
        return view('nostr-plugins');
    }

}
