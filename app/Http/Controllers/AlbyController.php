<?php

namespace App\Http\Controllers;

use Exception;
use Illuminate\Http\Request;
use Svix\Webhook;

class AlbyController extends Controller
{
    public function handleInvoiceSettled(Request $request)
    {
        $payload = $request->getContent();
        $svixSignature = $request->header('Svix-Signature');

        try {
            $webhook = new Webhook(env('ALBY_WEBHOOK_SECRET')); // Replace with your actual endpoint_secret from Alby
            $verifiedPayload = $webhook->verify($payload, [
                'svix-id' => $request->header('Svix-Id'),
                'svix-timestamp' => $request->header('Svix-Timestamp'),
                'svix-signature' => $svixSignature,
            ]);

            // If the request is verified, proceed to process the payload
            $invoiceData = json_decode($verifiedPayload, true);
            // Process the invoice data (e.g., credit user's account)

            return response()->json(['message' => 'Success'], 200);
        } catch (Exception $e) {
            // Log or handle verification failure
            return response()->json(['message' => 'Verification failed'], 403);
        }
    }
}
