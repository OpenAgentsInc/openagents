<?php

namespace App\Models;

use App\Enums\UserRole;
use App\Traits\Payable;
use Exception;
use Illuminate\Contracts\Auth\MustVerifyEmail;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Illuminate\Support\Facades\DB;
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
        return env('APP_ENV') === 'local' || $this->subscribed('default') || $this->isAdmin();
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

    public function plugins(): HasMany
    {
        return $this->hasMany(Plugin::class);
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

    public function canBypassPayments(): bool
    {
        return env('ADMIN_CAN_BYPASS_PAYMENTS', env('APP_ENV') === 'staging' && $this->isAdmin());
    }

    public function lightningAddresses()
    {
        return $this->hasMany(LightningAddress::class);
    }

    public function getLightningAddress(): string
    {

        $lnDomain = env('LIGHTNING_ADDR_DOMAIN', env('APP_ENV') === 'staging' ? 'staging.openagents.com' : 'openagents.com');

        // first we look for a "vanity" address
        $vanityAddress = $this->lightningAddresses()
            ->where('vanity', true)
            ->orderByDesc('updated_at')
            ->first();

        if ($vanityAddress) {
            return strtolower($vanityAddress->address);
        }

        return DB::transaction(function () use ($lnDomain) {

            // if not found we look for a "system" address
            $systemAddress = $this->lightningAddresses()
                ->where('vanity', false)
                ->orderByDesc('updated_at')
                ->first();

            // if not found we create it
            if (! $systemAddress) {
                // we will try to assign a system address, however there might be a vanity address that uses the same identifier
                // if that's the case we add a number and increment by 1 until we find a free address
                $i = 0;
                do {
                    $newSystemAddress = strtolower($this->username);
                    if ($i > 0) { // first attempt is without the number
                        $newSystemAddress .= $i;
                    }
                    $newSystemAddress .= '@';
                    $newSystemAddress .= $lnDomain;
                    $systemAddress = LightningAddress::whereRaw('LOWER(address) = ?', [$newSystemAddress])->first();
                    if (! $systemAddress) { // found a free address
                        // Use the address and break the loop
                        $systemAddress = $this->lightningAddresses()->create([
                            'address' => $newSystemAddress,
                            'vanity' => false,
                        ]);
                        break;
                    }
                    $i++;
                } while (true);
            }

            return strtolower($systemAddress->address);
        }, 5);
    }

    /**
     * Assign a vanity address to the user
     * If the user already used the same vanity address it will just reuse it
     * If another user is using the same vanity address it will return an error
     * If the vanity address is an username it will return an error as if the address was already used (this is to make it backward compatible)
     *
     * @param  string  $identifier  The first part of the address before @ (xxx@openagents.com)
     */
    public function setVanityAddress(string $identifier): string
    {
        if (strpos($identifier, '@') !== false) {
            $identifier = explode('@', $identifier)[0];
        }
        $lnDomain = env('LIGHTNING_ADDR_DOMAIN', env('APP_ENV') === 'staging' ? 'staging.openagents.com' : 'openagents.com');

        // Only A-Z, a-z, 0-9, _, -, . are allowed in the identifier, replace everything else with _
        $identifier = preg_replace('/[^A-Za-z0-9_\-\.]/', '_', $identifier);

        // to lower case
        $identifier = strtolower($identifier);

        return DB::transaction(function () use ($identifier, $lnDomain) {

            $vanityAddress = $identifier.'@'.$lnDomain;

            // let's check if the address is already in use
            $existingAddress = LightningAddress::whereRaw('LOWER(address) = ?', [$vanityAddress])->first();

            if ($existingAddress) {
                // its the same user, so we just update the updated_at date to move the address on top and then return
                if ($existingAddress->user_id == $this->id) {
                    $existingAddress->touch();
                    $existingAddress->save();

                    return strtolower($vanityAddress);
                } else {
                    throw new Exception('Address already in use');
                }
            }

            // now let's check if the $identifier is an existing username
            $existingUser = User::whereRaw('LOWER(username) = ?', [$identifier])->first();
            if ($existingUser) {
                throw new Exception('Address already in use');
            }

            // Check if there is a vanity address created in the last 6 hours
            if (! $this->isAdmin()) {
                $hasRecentVanityAddress = $this->lightningAddresses()
                    ->where('vanity', true)
                    ->where('created_at', '>=', now()->subHours(6))
                    ->exists();
            }

            if ($hasRecentVanityAddress) {
                throw new Exception('You have already changed your address recently. Please wait a few hours before trying again.');
            }

            // Address is free, let's use it
            $this->lightningAddresses()->create([
                'address' => $vanityAddress,
                'vanity' => true,
            ]);

            return $vanityAddress;
        }, 5);
    }

    public static function fromLightningAddress(string $addr): ?self
    {
        if (strpos($addr, '@') === false) {
            $lnDomain = env('LIGHTNING_ADDR_DOMAIN', env('APP_ENV') === 'staging' ? 'staging.openagents.com' : 'openagents.com');
            $addr = $addr.'@'.$lnDomain;
        }

        // find the user by the address (system or vanity)
        $addr = LightningAddress::whereRaw('LOWER(address) = LOWER(?)', [$addr])->first();
        if (! $addr) {
            return null;
        }

        return $addr->user;
    }
}
