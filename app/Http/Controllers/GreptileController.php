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

    public function getRepositoryStatus($encodedRepositoryId)
    {
        $decodedData = json_decode(base64_decode($encodedRepositoryId), true);

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
