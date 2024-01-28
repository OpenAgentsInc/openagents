<?php

namespace App\Services;

use App\Services\Alby\Client;

class L402
{
    public function __construct()
    {
        $this->wallet = new Client(env('ALBY_ACCESS_TOKEN'));
        // die and dump the length of the access token
        dd(config('env'));
        dd(strlen(env('TOGETHER_API_KEY')));

        dd($this->wallet->getInfo());
    }
}
