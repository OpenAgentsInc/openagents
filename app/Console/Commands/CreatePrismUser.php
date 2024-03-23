<?php

namespace App\Console\Commands;

use App\Services\PrismService;
use Illuminate\Console\Command;

class CreatePrismUser extends Command
{
    protected $signature = 'prism:create-user {lnAddress?}';

    protected $description = 'Creates a new Prism user with an optional Lightning Address';

    private $prismService;

    public function __construct(PrismService $prismService)
    {
        parent::__construct();
        $this->prismService = $prismService;
    }

    public function handle()
    {
        $lnAddress = $this->argument('lnAddress');
        // For now, let's not pass nwcConnection details through the command,
        // and just use what's defined in PrismService (which defaults to .env NWC_URL)
        $result = $this->prismService->createUser($lnAddress);

        if (isset($result['error'])) {
            $this->error('Failed to create user: '.$result['message']);
        } else {
            $this->info('User created successfully.');
            // Optionally print the full result for debugging/verification
            $this->line(print_r($result, true));
        }
    }
}
