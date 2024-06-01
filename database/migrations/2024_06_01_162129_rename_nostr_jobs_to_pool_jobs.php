<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::rename('nostr_jobs', 'pool_jobs');
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::rename('pool_jobs', 'nostr_jobs');
    }
};
