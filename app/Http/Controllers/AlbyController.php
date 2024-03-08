<?php

namespace App\Http\Controllers;

use App\Models\Invoice;
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
            $webhook = new Webhook(env('ALBY_WEBHOOK_SECRET'));
            $verifiedPayload = $webhook->verify($payload, [
                'svix-id' => $request->header('Svix-Id'),
                'svix-timestamp' => $request->header('Svix-Timestamp'),
                'svix-signature' => $svixSignature,
            ]);

            // Process the invoice data (e.g., credit user's account)
            Log::info($verifiedPayload);

            // Find the invoice by its identifier and update it
            $invoice = Invoice::where('identifier', $verifiedPayload['identifier'])->first();

            if ($invoice) {
                $invoice->update([
                    'destination_pubkey' => $verifiedPayload['destination_pubkey'] ?? null,
                    'preimage' => $verifiedPayload['preimage'] ?? null,
                    'settled' => $verifiedPayload['settled'] ?? false,
                    'settled_at' => $verifiedPayload['settled_at'] ?? null,
                    'state' => $verifiedPayload['state'] ?? null,
                ]);
                Log::info('Invoice updated successfully', ['identifier' => $verifiedPayload['identifier']]);
            } else {
                Log::warning('Invoice not found for identifier: '.$verifiedPayload['identifier']);
            }

            return response()->json(['message' => 'Success']);
        } catch (Exception $e) {
            // Log or handle verification failure
            return response()->json(['message' => 'Verification failed'], 403);
        }
    }
}
