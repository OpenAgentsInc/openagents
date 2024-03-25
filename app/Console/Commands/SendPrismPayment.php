<?php

namespace App\Console\Commands;

use App\Models\User;
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
        // Ensure the sender exists or create them
        $sender = User::firstOrCreate(
            ['prism_user_id' => '68f5d9c3-9260-4fdc-b29f-8e5e8edcb849'],
            ['name' => 'OpenAgents', 'email' => 'chris+oa@openagents.com'] // Adjust default attributes as needed
        );

        $recipients = [
            ['rblb@blink.sv', '3cf9098c-283f-4843-8ee8-ada9a907a75f', 50],
            ['mcdonald55@bitnob.io', '9a96fe41-30a6-4b84-96a2-7184f107be96', 50],
        ];

        // Ensure each recipient exists
        foreach ($recipients as $recipient) {
            User::firstOrCreate(
                ['prism_user_id' => $recipient[1]],
                ['name' => $recipient[0], 'email' => $recipient[0], 'ln_address' => $recipient[0]],  // Adjust default attributes as needed
            );
        }

        // Prism recipient object is $recipients minus the first element
        $prism_recipients = array_map(fn ($recipient) => array_slice($recipient, 1), $recipients);

        $result = $this->prismService->sendPayment(100, 'SAT', $prism_recipients);

        $this->info('Payment response: '.print_r($result, true));
    }
}
