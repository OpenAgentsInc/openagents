<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('l402_credentials', function (Blueprint $table) {
            $table->id();
            $table->string('host');
            $table->string('scope');
            $table->text('macaroon');
            $table->text('preimage');
            $table->timestamp('expires_at');
            $table->timestamps();

            $table->unique(['host', 'scope']);
            $table->index('expires_at');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('l402_credentials');
    }
};
