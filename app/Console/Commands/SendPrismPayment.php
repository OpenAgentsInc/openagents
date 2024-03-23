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
            ['rblb@blink.sv', 50], // Example recipient format
            ['mcdonald55@bitnob.io', 50],
        ];

        $result = $this->prismService->sendPayment(100, 'SAT', $recipients);

        $this->info('Payment response: '.print_r($result, true));
    }
}
