        // Assert response status code is 200 or 204 (No Content)
        $response->assertStatus(200);
        
        // Assert that the memory no longer exists in the database
        $this->assertDatabaseMissing('memories', ['id' => $memory->id]);
    }

    /**
     * Test to ensure destroy returns an error when given an invalid id.
     *
     * @return void
     */
    public function testDestroyInvalidId()
    {
        // Make DELETE request to destroy route with invalid id
        $response = $this->delete('/memories/999');

        // Assert response status code is 404
        $response->assertStatus(404);
    }
}
