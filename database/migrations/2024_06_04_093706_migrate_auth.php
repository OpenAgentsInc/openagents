<?php

use App\Models\NostrAccount;
use App\Models\User;
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;
use swentel\nostr\Key\Key;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up()
    {
        Schema::table('users', function (Blueprint $table) {
            if (! Schema::hasColumn('users', 'external_id')) {
                $table->string('external_id')->nullable()->after('email');
            }
            if (! Schema::hasColumn('users', 'auth_provider')) {
                $table->string('auth_provider')->nullable()->after('email');
            }

        });

        DB::table('users')->whereNotNull('username')->update(['external_id' => DB::raw('username')]);

        $nostrAccounts = NostrAccount::all();
        foreach ($nostrAccounts as $nostrAccount) {
            $user = User::find($nostrAccount->user_id);
            if ($user) {
                $user->update(['external_id' => $nostrAccount->pubkey]);

                $key = new Key();
                $bech32_public = $key->convertPublicKeyToBech32($nostrAccount->pubkey);
                $user->update(['username' => $bech32_public]);

                $user->update(['auth_provider' => 'nostr']);
                $user->save();
            }
        }

        DB::table('users')
            ->whereNull('auth_provider')
            ->update(['auth_provider' => 'X']);

    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->dropColumn('external_id');
            $table->dropColumn('auth_provider');
        });

    }
};
