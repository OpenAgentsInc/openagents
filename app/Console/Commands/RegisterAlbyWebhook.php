<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\Http;

class RegisterAlbyWebhook extends Command
{
    protected $signature = 'alby:register-webhook';

    protected $description = 'Registers a webhook endpoint with Alby and displays the secret key';

    public function handle()
    {
        $url = 'https://api.getalby.com/webhook_endpoints'; // Use the actual Alby API URL
        $apiKey = env('ALBY_ACCESS_TOKEN'); // Store your Alby API key in the .env file for security

        $response = Http::withHeaders([
            'Authorization' => "Bearer {$apiKey}",
            'Content-Type' => 'application/json',
        ])->post($url, [
            'url' => 'https://staging.openagents.com/webhooks/alby/invoice-settled', // Your webhook URL
            'filter_types' => ['invoice.incoming.settled'],
        ]);

        if ($response->successful()) {
            $data = $response->json();
            $endpointSecret = $data['endpoint_secret'];

            $this->info("Webhook registered successfully. Endpoint Secret: {$endpointSecret}");

            // Optionally, save the endpoint secret in your .env file or a secure location
            // Make sure to implement logic here to securely store the secret key if needed
        } else {
            $this->error('Failed to register the webhook endpoint. Please check your API key and endpoint URL.');
        }
    }
}
