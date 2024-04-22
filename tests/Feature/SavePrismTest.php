<?php

use App\Services\PrismService;

test('save prism single+multi records to db', function () {

    // Assert there are no PrismSinglePayments or PrismMultiPayments in the db
    $this->assertDatabaseCount('prism_single_payments', 0);
    $this->assertDatabaseCount('prism_multi_payments', 0);

    $response = [
        'prismId' => 'c580c889-bf3d-46bf-b24f-ffb9521dc9e7',
        'payments' => [
            [
                'id' => 'be5d706e-f853-4bd4-9305-d3ed8da8be3e',
                'createdAt' => 1713816394,
                'updatedAt' => null,
                'expiresAt' => null,
                'senderId' => '68f5d9c3-9260-4fdc-b29f-8e5e8edcb849',
                'receiverId' => 'e1d22ee1-3bff-41ff-9ceb-aced4247bda8',
                'amountMsat' => 50000,
                'status' => 'sending',
                'resolvedAt' => null,
                'resolved' => false,
                'prismPaymentId' => null,
                'bolt11' => null,
                'preimage' => null,
                'failureCode' => null,
                'type' => 'DEFAULT',
                'reason' => 'Awaiting NWC response',
            ],
        ],
    ];

    $prismService = new PrismService();
    $prismService->savePrism($response);

    // Assert there is 1 PrismSinglePayment and 1 PrismMultiPayment in the db
    $this->assertDatabaseCount('prism_single_payments', 1);
    $this->assertDatabaseCount('prism_multi_payments', 1);

    // Assert fields match
    $this->assertDatabaseHas('prism_single_payments', [
        'payment_id' => 'be5d706e-f853-4bd4-9305-d3ed8da8be3e',
        'prism_multi_payment_id' => 1,
    ]);
});
