<?php

namespace App\Console\Commands;

use App\Models\Payment;
use App\Models\User;
use Illuminate\Console\Command;

class CreateDemoPayment extends Command
{
    protected $signature = 'pay:demo';

    protected $description = 'Creates a demo payment with dummy data for testing';

    public function handle()
    {
        // Dummy sender and receiver data
        $senderPrismId = 'sender-12345-67890';
        $receiverPrismId = 'receiver-12345-67890';

        // Ensure the sender and receiver exist or create them
        $sender = User::firstOrCreate(
            ['prism_user_id' => $senderPrismId],
            ['name' => 'Demo Sender', 'email' => 'sender@example.com']
        );

        $receiver = User::firstOrCreate(
            ['prism_user_id' => $receiverPrismId],
            ['name' => 'Demo Receiver', 'email' => 'receiver@example.com']
        );

        // Create a Payment object with dummy data
        $payment = Payment::create([
            'receiver_id' => $receiver->id,
            'sender_id' => $sender->id,
            'prism_id' => 'demo-'.now()->timestamp,
            'prism_created_at' => now()->timestamp,
            'prism_updated_at' => now()->timestamp,
            'expires_at' => now()->addDay()->timestamp,
            'receiver_prism_id' => $receiverPrismId,
            'sender_prism_id' => $senderPrismId,
            'type' => 'DEFAULT',
            'amount_msat' => 100000, // 100,000 millisatoshis (0.1 sat)
            'status' => 'pending',
            'resolved' => false,
            // 'resolved_at' => null, // Optionally set if resolved
            'prism_payment_id' => 'demo-payment-'.now()->timestamp,
            'bolt11' => 'lnbc1demo...',
            'preimage' => '000102030405060...',
            'failure_code' => null, // No failure since it's a demo
        ]);

        $this->info('Demo payment created successfully.');
        $this->info('Total payments in database: '.Payment::count());
    }
}
