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
        Schema::table('plugins', function (Blueprint $table) {
            $table->boolean('enabled')->default(true);
            $table->json('pending_revision')->nullable();
            $table->string('pending_revision_reason')->nullable();
            $table->string('suspended')->default('Pending approval');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('plugins', function (Blueprint $table) {
            $table->dropColumn('enabled');
            $table->dropColumn('pending_revision');
            $table->dropColumn('pending_revision_reason');
            $table->dropColumn('suspended');
        });
    }
};
