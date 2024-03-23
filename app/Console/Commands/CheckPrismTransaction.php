<?php

namespace App\Console\Commands;

use App\Services\PrismService;
use Illuminate\Console\Command;

class CheckPrismTransaction extends Command
{
    protected $signature = 'prism:check-transaction {transactionId}';

    protected $description = 'Checks the details of a Prism transaction';

    protected $prismService;

    public function __construct(PrismService $prismService)
    {
        parent::__construct();
        $this->prismService = $prismService;
    }

    public function handle()
    {
        $transactionId = $this->argument('transactionId');

        $transactionDetails = $this->prismService->getTransactionDetails($transactionId);

        if (isset($transactionDetails['error'])) {
            $this->error('Failed to retrieve transaction details: '.$transactionDetails['message']);
        } else {
            $this->info('Transaction details retrieved successfully:');
            // Display the transaction details
            // You might want to format this output or extract specific details to display
            print_r($transactionDetails);
        }
    }
}
