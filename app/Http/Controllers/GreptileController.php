<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Services\GreptileService;

class GreptileController extends Controller
{
    protected $greptileService;

    public function __construct(GreptileService $greptileService)
    {
        $this->greptileService = $greptileService;
    }

    public function indexRepository(Request $request)
    {
        $request->validate([
            'remote' => 'required|string',
            'repository' => 'required|string',
            'branch' => 'required|string',
        ]);

        $result = $this->greptileService->indexRepository(
            $request->input('remote'),
            $request->input('repository'),
            $request->input('branch')
        );

        if (isset($result['error'])) {
            return response()->json(['error' => $result['error']], 500);
        }

        return response()->json($result);
    }

    public function getRepositoryStatus()
    {
        // Instead grab the encodedRepositoryId from the request
        $encodedRepositoryId = request()->query('repositoryId');
        \Log::info('Encoded repository ID', ['encodedRepositoryId' => $encodedRepositoryId]);

        // Decode the repository ID which is URL-encoded in the format remote:branch:owner/repository
        $decodedData = urldecode($encodedRepositoryId); // github:v2:openagentsinc/openagents
        \Log::info('Decoded repository ID', ['decodedData' => $decodedData]);
        $exploded = explode(':', $decodedData);
        $decodedData = [
            'remote' => $exploded[0],
            'branch' => $exploded[1],
            'repository' => $exploded[2],
        ];

        if (!$decodedData || !isset($decodedData['remote'], $decodedData['repository'], $decodedData['branch'])) {
            return response()->json(['error' => 'Invalid repository ID'], 400);
        }

        $result = $this->greptileService->getRepositoryStatus(
            $decodedData['remote'],
            $decodedData['repository'],
            $decodedData['branch']
        );

        if (isset($result['error'])) {
            return response()->json(['error' => $result['error']], 500);
        }

        return response()->json($result);
    }

}
