<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class UserSparkWallet extends Model
{
    /** @use HasFactory<\Database\Factories\UserSparkWalletFactory> */
    use HasFactory;

    /**
     * @var string
     */
    protected $table = 'user_spark_wallets';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'user_id',
        'wallet_id',
        'mnemonic',
        'spark_address',
        'lightning_address',
        'identity_pubkey',
        'last_balance_sats',
        'status',
        'provider',
        'last_error',
        'meta',
        'last_synced_at',
    ];

    /**
     * @var list<string>
     */
    protected $hidden = [
        'mnemonic',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'mnemonic' => 'encrypted',
            'meta' => 'array',
            'last_synced_at' => 'datetime',
            'last_balance_sats' => 'integer',
        ];
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }
}
