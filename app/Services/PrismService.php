<?php

namespace App\Services;

use App\Models\PrismMultiPayment;
use App\Models\PrismSinglePayment;
use Exception;
use Illuminate\Support\Facades\Http;

class PrismService
{
    protected $baseUrl = 'https://api.makeprisms.com/v0';

    protected $apiKey; // Assume you've stored your API key somewhere secure

    public function __construct()
    {
        $this->apiKey = env('PRISM_API_KEY');
    }

    public function createUser($lnAddress = null, $nwcConnection = null)
    {
        $payload = [];

        if (! is_null($lnAddress)) {
            $payload['lnAddress'] = $lnAddress;
        }

        if (! is_null($nwcConnection)) {
            // Assuming $nwcConnection is an array with the necessary fields
            $payload['nwcConnection'] = $nwcConnection;
        } else {
            // Use the NWC_URL from .env if no nwcConnection is explicitly passed
            $nwcUrl = env('NWC_URL');
            $payload['nwcConnection'] = [
                'nwcUrl' => $nwcUrl,
                'connectorType' => 'nwc.alby',
                'connectorName' => 'bitcoin-connect',
            ];
        }

        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/user", $payload);

        return $response->json();
    }

    public function updateUserLnAddress($userId, $lightningAddress)
    {
        $response = Http::withToken($this->apiKey)
            ->patch("{$this->baseUrl}/user/{$userId}", [
                'lnAddress' => $lightningAddress,
            ]);

        return $response->json();
    }

    public function sendPayment($amount, array $recipients)
    {
        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/payment/prism", [
                //                'senderId' => '67c2cc15-d90f-4af5-b16f-06cebd9e8e5d', // atlantispleb
                'senderId' => '68f5d9c3-9260-4fdc-b29f-8e5e8edcb849', // openagents
                'amount' => $amount,
                'currency' => 'SAT',
                'prism' => $recipients,
            ]);

        $json = $response->json();

        if ($response->status() === 200) {
            $this->savePrism($json);
        } else {
            dd($json);
        }

        return $json;
    }

    public function savePrism($prismResponse)
    {
        try {
            $prismMultiPayment = new PrismMultiPayment();
            $prismMultiPayment->prism_id = $prismResponse['prismId'];
            $prismMultiPayment->save();

            foreach ($prismResponse['payments'] as $payment) {
                $prismSinglePayment = new PrismSinglePayment();
                $prismSinglePayment->payment_id = $payment['id'];
                $prismSinglePayment->prism_multi_payment_id = $prismMultiPayment->id;
                $prismSinglePayment->prism_id = $prismResponse['prismId'];
                $prismSinglePayment->expires_at = $payment['expiresAt'];
                $prismSinglePayment->sender_id = $payment['senderId'];
                $prismSinglePayment->receiver_id = $payment['receiverId'];
                $prismSinglePayment->amount_msat = $payment['amountMsat'];
                $prismSinglePayment->status = $payment['status'];
                $prismSinglePayment->resolved_at = $payment['resolvedAt'];
                $prismSinglePayment->resolved = $payment['resolved'];
                $prismSinglePayment->prism_payment_id = $payment['prismPaymentId'];
                $prismSinglePayment->bolt11 = $payment['bolt11'];
                $prismSinglePayment->preimage = $payment['preimage'];
                $prismSinglePayment->failure_code = $payment['failureCode'];
                $prismSinglePayment->type = $payment['type'];
                $prismSinglePayment->reason = $payment['reason'];
                $prismSinglePayment->save();
            }
        } catch (Exception $e) {
            // Log the error
            dd($e->getMessage());
            // Return a response indicating failure
        }

    }

    public function getTransactionDetails($transactionId)
    {
        // Replace with the actual endpoint for fetching transaction details
        // Assuming the endpoint follows the pattern /transaction/{transactionId}
        $response = Http::withToken($this->apiKey)
            ->get("{$this->baseUrl}/payment/{$transactionId}");

        return $response->json();
    }
}
