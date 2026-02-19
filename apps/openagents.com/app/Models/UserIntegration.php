<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class UserIntegration extends Model
{
    use HasFactory;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'user_id',
        'provider',
        'status',
        'encrypted_secret',
        'secret_fingerprint',
        'secret_last4',
        'metadata',
        'connected_at',
        'disconnected_at',
    ];

    /**
     * @var list<string>
     */
    protected $hidden = [
        'encrypted_secret',
        'secret_fingerprint',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'encrypted_secret' => 'encrypted',
            'metadata' => 'array',
            'connected_at' => 'datetime',
            'disconnected_at' => 'datetime',
        ];
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function audits(): HasMany
    {
        return $this->hasMany(UserIntegrationAudit::class, 'user_integration_id');
    }
}
