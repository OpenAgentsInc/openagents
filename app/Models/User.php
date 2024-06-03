<?php

namespace App\Models;

use App\Traits\Payable;
use Illuminate\Contracts\Auth\MustVerifyEmail;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Laravel\Cashier\Billable;
use Laravel\Jetstream\HasProfilePhoto;
use Laravel\Sanctum\HasApiTokens;

class User extends Authenticatable implements MustVerifyEmail
{
    use Billable;
    use HasApiTokens;
    use HasFactory;
    use HasProfilePhoto;
    use Notifiable;
    use Payable;

    /**
     * The attributes that are mass assignable.
     *
     * @var array<int, string>
     */
    protected $fillable = [
        'name',
        'username',
        'email',
        'password',
        'profile_photo_path',
        'default_model',
        'system_prompt',
        'autoscroll',
        'lightning_address',
    ];

    /**
     * The attributes that should be hidden for serialization.
     *
     * @var array<int, string>
     */
    protected $hidden = [
        'password',
        'remember_token',
    ];

    /**
     * The accessors to append to the model's array form.
     *
     * @var array<int, string>
     */
    protected $appends = [
        'profile_photo_url',
        //        'default_model',
    ];

    public function dateForHumans()
    {
        return $this->created_at->format(
            $this->created_at->year === now()->year ?
                'M d, g:i A' :
                'M d Y, g:i A'
        );
    }

    // has an attribute sats_balance that gets the bitcoin balance divided by 1000
    //    public function getSatsBalanceAttribute(): int
    //    {
    //        return (int) $this->checkBalance(Currency::BTC) / 1000;
    //    }

    public function getIsProAttribute(): bool
    {
        return $this->isPro();
    }

    public function isPro(): bool
    {
        return env('APP_ENV') === 'local' || $this->subscribed('default');
    }

    public function isAdmin(): bool
    {
        return $this->username === 'AtlantisPleb';
    }

    public function payins(): HasMany
    {
        return $this->hasMany(Payin::class);
    }

    public function agents(): HasMany
    {
        return $this->hasMany(Agent::class);
    }

    public function threads(): HasMany
    {
        return $this->hasMany(Thread::class);
    }

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class);
    }

    public function nostrAccount(): HasOne
    {
        return $this->hasOne(NostrAccount::class);
    }

    /**
     * Get the attributes that should be cast.
     *
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'password' => 'hashed',
        ];
    }
}
