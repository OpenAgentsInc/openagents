<?php

namespace App\Jobs;

use App\Models\User;
use Exception;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;

class RefreshNostrMetaForUser implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public $tries = 3;

    protected $pubkey;

    /**
     * Create a new job instance.
     */
    public function __construct($pubkey)
    {
        $this->pubkey = $pubkey;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        try {
            DB::transaction(function () {
                $user = User::where('auth_provider', 'nostr')
                    ->where('external_id', $this->pubkey)
                    ->lockForUpdate()
                    ->first();

                if ($user) {
                    $npubResolver = str_replace('{{$npub}}', $user->username, config('nostr.npub_resolver'));
                    $npubDataResponse = Http::timeout(5)->get($npubResolver);
                    $npubData = $npubDataResponse->json();
                    if (isset($npubData['items']) && isset($npubData['items'][1])) {
                        $npubData = $npubDataResponse->json()['items'][1];
                        $npubContent = json_decode($npubData['content'] ?? '{}', true);
                        $user->profile_photo_path = $npubContent['picture'] ?? '/images/nostrich.jpeg';
                        $user->name = $npubContent['name'] ?? substr($this->pubkey, 0, 8);
                        $user->save();
                    }
                }
            });
        } catch (Exception $e) {
            Log::error('Error fetching npub data: '.$e->getMessage());
            $this->release(2);
        }
    }
}
