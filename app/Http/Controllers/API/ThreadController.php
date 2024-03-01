<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Services\ThreadService;
use Illuminate\Http\Request;

class ThreadController extends Controller
{
    protected ThreadService $threadService;

    public function __construct(ThreadService $threadService)
    {
        $this->threadService = $threadService;
    }

    /**
     * Display a listing of the resource.
     */
    public function index()
    {
        //
    }

    /**
     * Store a newly created resource in storage.
     */
    public function store(Request $request)
    {
        try {
            $thread = $this->threadService->createThread(
                $request->input('agent_id')
            );

            return response()->json(['success' => true, 'data' => $thread]);
        } catch (Exception $e) {
            return response()->json(['message' => $e->getMessage()], 500);
        }
    }

    /**
     * Display the specified resource.
     */
    public function show(string $id)
    {
        //
    }

    /**
     * Update the specified resource in storage.
     */
    public function update(Request $request, string $id)
    {
        //
    }

    /**
     * Remove the specified resource from storage.
     */
    public function destroy(string $id)
    {
        //
    }
}
