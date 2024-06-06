<?php

namespace App\Models;

use App\Enums\UserRole;
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
        'external_id',
        'auth_provider',
        'name',
        'username',
        'email',
        'password',
        'profile_photo_path',
        'default_model',
        'system_prompt',
        'autoscroll',
        'lightning_address',
        'role', // 0 = user, 1 = moderator, 2 = admin
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

    /**
     * @deprecated Use getRole()>=UserRole::SUPER_ADMIN instead
     */
    public function isSuperAdmin(): bool
    {
        return $this->getRole()->value >= UserRole::SUPER_ADMIN->value;
    }

    /**
     * @deprecated Use getRole()>=UserRole::MOD instead
     */
    public function isModerator(): bool
    {
        return $this->getRole()->value >= UserRole::MOD->value;
    }

    /**
     * @deprecated Use getRole()>=UserRole::ADMIN instead
     */
    public function isAdmin(): bool
    {
        return $this->getRole()->value >= UserRole::ADMIN->value;
    }

    public function getRole(): UserRole
    {
        $forceSuperAdmin = 'AtlantisPleb';

        $forceAdmin = 'npub1klt4m7gsqtx0e5erq9snquk8g2sw79mwm6kjau02nufnny99pcysd4kr0p';

        if ($forceSuperAdmin) {
            if ($this->username == $forceSuperAdmin) {
              
                return UserRole::SUPER_ADMIN;
            }
            // When super admin is forced, all other super admins are downgraded to admin
            if ($this->role > UserRole::ADMIN->value) {
                return UserRole::ADMIN;
            }

        }

        if ($forceAdmin) {
            if ($this->username == $forceAdmin) {
                return UserRole::ADMIN;
            }
        }

        return UserRole::fromInt($this->role ?? 0);
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
