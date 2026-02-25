<?php

namespace PostHog;

class SizeLimitedHash
{
    /**
     * @var int
     */
    private $size;

    /**
     * @var array
     */
    private $mapping;

    public function __construct($size)
    {
        $this->size = $size;
        $this->mapping = [];
    }

    public function add($key, $element)
    {

        if (count($this->mapping) >= $this->size) {
            $this->mapping = [];
        }

        if (array_key_exists($key, $this->mapping)) {
            array_push($this->mapping, $element);
        } else {
            $this->mapping[$key] = [$element];
        }
    }

    public function contains($key, $element)
    {
        if (array_key_exists($key, $this->mapping) && array_key_exists($element, $this->mapping[$key])) {
            return true;
        }

        return false;
    }

    public function count()
    {
        return count($this->mapping);
    }
}
