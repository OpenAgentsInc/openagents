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

        $result = $this->prismService->createUser($lnAddress);

        if (isset($result['error'])) {
            $this->error('Failed to create user: '.$result['message']);
        } else {
            print_r($result);
            //            $this->info('User created successfully. User ID: '.$result['userId']);
            // Display other relevant info if needed
        }
    }
}
