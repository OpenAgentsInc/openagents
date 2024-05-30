<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class PluginSecretResource extends JsonResource
{

    /**
     * The "data" wrapper that should be applied.
     *
     * @var string
     */
    public static $wrap = null;


    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        // return parent::toArray($request);


        // Decode the secrets field
        $secrets = json_decode($this->secrets, true);
        $secretsArray = [];

        if (is_array($secrets)) {
            foreach ($secrets as $secret) {
                $secretsArray[$secret['key']] = $secret['value'];
            }
        }


        return [
            $this->file_link => $secretsArray
        ];
    }
}
