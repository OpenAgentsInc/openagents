<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('users', function (Blueprint $table) {
            $table->id();

            // Email/pass
            $table->string('email')->unique()->nullable();
            $table->timestamp('email_verified_at')->nullable();
            $table->string('password')->nullable();

            // Bio
            $table->string('name')->nullable();

            // Social
            $table->string('nostr_pubkey')->unique()->nullable();
            $table->bigInteger('github_id')->unique()->nullable();
            $table->string('github_nickname')->nullable();
            $table->string('github_avatar')->nullable();
            $table->bigInteger('twitter_id')->unique()->nullable();
            $table->string('twitter_nickname')->nullable();
            $table->string('twitter_avatar')->nullable();

            // Payments
            $table->integer('balance')->default(0); // sats
            $table->string('lightning_address')->nullable();

            $table->rememberToken();
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('users');
    }
};
