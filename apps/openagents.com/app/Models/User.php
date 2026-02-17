<?php

namespace App\Models;

// use Illuminate\Contracts\Auth\MustVerifyEmail;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Illuminate\Support\Facades\DB;
use Laravel\Sanctum\HasApiTokens;

class User extends Authenticatable
{
    /** @use HasFactory<\Database\Factories\UserFactory> */
    use HasApiTokens, HasFactory, Notifiable;

    /**
     * The attributes that are mass assignable.
     *
     * @var list<string>
     */
    protected $fillable = [
        'name',
        'email',
        'handle',
        'workos_id',
        'avatar',
    ];

    /**
     * The attributes that should be hidden for serialization.
     *
     * @var list<string>
     */
    protected $hidden = [
        'workos_id',
        'remember_token',
    ];

    /**
     * Get the attributes that should be cast.
     *
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'email_verified_at' => 'datetime',
            'password' => 'hashed',
        ];
    }

    protected static function booted(): void
    {
        static::creating(function (self $user): void {
            if (! is_string($user->handle) || trim($user->handle) === '') {
                $seed = (string) ($user->email ?: $user->name ?: 'user');
                $user->handle = self::generateUniqueHandle($seed);
            }
        });
    }

    public function sparkWallet(): HasOne
    {
        return $this->hasOne(UserSparkWallet::class);
    }

    public function autopilots(): HasMany
    {
        return $this->hasMany(Autopilot::class, 'owner_user_id');
    }

    public function shouts(): HasMany
    {
        return $this->hasMany(Shout::class);
    }

    public function sentWhispers(): HasMany
    {
        return $this->hasMany(Whisper::class, 'sender_id');
    }

    public function receivedWhispers(): HasMany
    {
        return $this->hasMany(Whisper::class, 'recipient_id');
    }

    /**
     * Get PostHog person properties for this user.
     *
     * @return array<string, mixed>
     */
    public function getPostHogProperties(): array
    {
        return [
            'email' => $this->email,
            'name' => $this->name,
            'handle' => $this->handle,
            'date_joined' => $this->created_at?->toISOString(),
        ];
    }

    public static function normalizeHandleBase(string $value): string
    {
        $candidate = strtolower(trim($value));

        if (str_contains($candidate, '@')) {
            $candidate = (string) explode('@', $candidate, 2)[0];
        }

        $candidate = preg_replace('/[^a-z0-9:_-]+/', '-', $candidate) ?? '';
        $candidate = trim($candidate, '-');

        if ($candidate === '') {
            return '';
        }

        return substr($candidate, 0, 64);
    }

    public static function generateUniqueHandle(string $seed, ?int $ignoreId = null): string
    {
        $base = self::normalizeHandleBase($seed);
        if ($base === '') {
            $base = 'user';
        }

        $handle = $base;
        $suffix = 1;

        while (self::handleExists($handle, $ignoreId)) {
            $suffixText = '-'.$suffix;
            $trimmed = substr($base, 0, max(1, 64 - strlen($suffixText)));
            $handle = $trimmed.$suffixText;
            $suffix++;
        }

        return $handle;
    }

    private static function handleExists(string $handle, ?int $ignoreId = null): bool
    {
        $query = DB::table('users')->where('handle', $handle);

        if ($ignoreId !== null) {
            $query->where('id', '!=', $ignoreId);
        }

        return $query->exists();
    }
}
