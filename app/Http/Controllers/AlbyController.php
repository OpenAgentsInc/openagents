<?php

namespace App\Http\Controllers;

use Exception;
use Illuminate\Http\Request;
use Log;
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

            // Process the invoice data (e.g., credit user's account)
            Log::info($verifiedPayload);

            return response()->json(['message' => 'Success'], 200);
        } catch (Exception $e) {
            // Log or handle verification failure
            return response()->json(['message' => 'Verification failed'], 403);
        }
    }
}
