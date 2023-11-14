<?php

use App\Models\Embedding;
use App\Services\QueenbeeGateway;
use Illuminate\Foundation\Inspiring;
use Illuminate\Support\Facades\Artisan;

/*
|--------------------------------------------------------------------------
| Console Routes
|--------------------------------------------------------------------------
|
| This file is where you may define all of your Closure based console
| commands. Each Closure is bound to a command instance allowing a
| simple approach to interacting with each command's IO methods.
|
*/

Artisan::command('inspire', function () {
    $this->comment(Inspiring::quote());
})->purpose('Display an inspiring quote');


Artisan::command('insert', function() {

    $sayings = [
        'Felines say meow',
        'Canines say woof',
        'Birds say tweet',
        'Humans say hello',
    ];

    $gateway = new QueenbeeGateway();
    $result = $gateway->createEmbedding($sayings);

    foreach ($sayings as $key=>$saying) {
        Embedding::query()->create([
            'embedding' => $result[$key]["embedding"],
            'metadata' => [
                'saying' => $saying,
            ]
        ]);
    }
});
