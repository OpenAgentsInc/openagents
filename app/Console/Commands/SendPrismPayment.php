<?php

namespace App\Console\Commands;

use App\Services\PrismService;
use Illuminate\Console\Command;

class SendPrismPayment extends Command
{
    protected $signature = 'prism:pay';

    protected $description = 'Sends a demo payment to two users via Prism API';

    protected $prismService;

    public function __construct(PrismService $prismService)
    {
        parent::__construct();
        $this->prismService = $prismService;
    }

    public function handle()
    {
        $recipients = [
            ['3cf9098c-283f-4843-8ee8-ada9a907a75f', 50], // rblb@blink.sv
            ['9a96fe41-30a6-4b84-96a2-7184f107be96', 50], // mcdonald55@bitnob.io
        ];

        $result = $this->prismService->sendPayment(100, 'SAT', $recipients);

        $this->info('Payment response: '.print_r($result, true));
    }
}
