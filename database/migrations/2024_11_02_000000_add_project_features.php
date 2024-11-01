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
        Schema::table('projects', function (Blueprint $table) {
            $table->text('custom_instructions')->nullable()->after('description');
            $table->text('context')->nullable()->after('custom_instructions');
            $table->json('settings')->nullable()->after('context');
            $table->string('status')->default('active')->after('settings');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('projects', function (Blueprint $table) {
            $table->dropColumn(['custom_instructions', 'context', 'settings', 'status']);
        });
    }
};