/**
 * Temporary file to test Healer integration
 * This file has intentional type errors to trigger the Healer
 */

// Type error: assigning string to number
const count: number = "this is wrong";

// Type error: calling non-existent method
const value = "hello".nonExistentMethod();

export const broken = true;
