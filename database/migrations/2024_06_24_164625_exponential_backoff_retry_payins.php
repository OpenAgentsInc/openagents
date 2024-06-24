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
        Schema::table('payins', function (Blueprint $table) {
            $table->timestamp('last_check')->after('updated_at')->useCurrent();
            $table->integer('retry_check')->default(0)->after('last_check');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('payins', function (Blueprint $table) {
            $table->dropColumn('last_check');
            $table->dropColumn('retry_check');
        });
    }
};
