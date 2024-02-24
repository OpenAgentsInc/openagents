<?php

namespace App\Contracts;

interface AIGateway
{
    public function inference($input);
    public function embedding($input);
}
