import { Data } from "effect";

// Validation error types
export class ValidationError extends Data.TaggedError("ValidationError")<{
  field: string;
  value: unknown;
  message: string;
  rule: string;
}> {}

export class SanitizationError extends Data.TaggedError("SanitizationError")<{
  field: string;
  originalValue: unknown;
  message: string;
}> {}

// Simple utility functions without Effect types to avoid TypeScript issues
export const sanitizeString = (input: string, maxLength: number = 1000): string => {
  if (typeof input !== 'string') {
    throw new Error('Input must be a string');
  }
  
  // Remove potentially dangerous characters
  const sanitized = input
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '') // Remove script tags
    .replace(/javascript:/gi, '') // Remove javascript: protocols
    .replace(/on\w+\s*=/gi, '') // Remove event handlers
    .trim();
  
  // Enforce length limits
  const truncated = sanitized.length > maxLength 
    ? sanitized.substring(0, maxLength) 
    : sanitized;
  
  return truncated;
};

// Rate limiting utilities
export const createRateLimiter = (maxRequests: number, windowMs: number) => {
  const requests = new Map<string, number[]>();
  
  return (identifier: string): boolean => {
    const now = Date.now();
    const userRequests = requests.get(identifier) || [];
    
    // Remove expired requests
    const validRequests = userRequests.filter(time => now - time < windowMs);
    
    if (validRequests.length >= maxRequests) {
      return false; // Rate limit exceeded
    }
    
    validRequests.push(now);
    requests.set(identifier, validRequests);
    return true;
  };
};

// Input validation without complex Effect patterns
export const isValidEmail = (email: string): boolean => {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
};

export const isValidGitHubUsername = (username: string): boolean => {
  const githubUsernameRegex = /^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?$/;
  return githubUsernameRegex.test(username) && username.length <= 39;
};