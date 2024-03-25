<?php

namespace App\Console\Commands;

use App\Models\Payment;
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

        if (isset($result['payments']) && is_array($result['payments'])) {
            foreach ($result['payments'] as $paymentData) {
                $receiver = User::firstOrCreate(
                    ['prism_user_id' => $paymentData['receiverId']],
                    ['name' => 'Unknown Receiver', 'email' => $paymentData['receiverId'].'@placeholder.com']
                );

                Payment::create([
                    'prism_id' => $paymentData['id'],
                    'prism_created_at' => $paymentData['createdAt'],
                    'prism_updated_at' => $paymentData['updatedAt'],
                    'expires_at' => $paymentData['expiresAt'],
                    'receiver_id' => $receiver->id,
                    'sender_id' => $sender->id,
                    'receiver_prism_id' => $paymentData['receiverId'],
                    'sender_prism_id' => $paymentData['senderId'],
                    'type' => $paymentData['type'],
                    'amount_msat' => $paymentData['amountMsat'],
                    'status' => $paymentData['status'],
                    'resolved' => $paymentData['resolved'],
                    'resolved_at' => $paymentData['resolved'] ? $paymentData['resolvedAt'] : null,
                    'prism_payment_id' => $paymentData['prismPaymentId'],
                    'bolt11' => $paymentData['bolt11'],
                    'preimage' => $paymentData['preimage'],
                    'failure_code' => $paymentData['failureCode'],
                ]);
            }
        }

        // Log how many Payments are in the database total
        $this->info('Total payments in database: '.Payment::count());
    }
}
