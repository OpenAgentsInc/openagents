/**
 * Network-related error classes
 */

import { ChatError, ChatErrorOptions } from './base-error';
import { ProviderType } from './provider-errors';

// Network error options
export interface NetworkErrorOptions extends Omit<ChatErrorOptions, 'category'> {
  url?: string;
  statusCode?: number;
  timeout?: number;
  retryable?: boolean;
  provider: ProviderType;
}

/**
 * Base class for network errors
 */
export class NetworkError extends ChatError {
  public readonly url?: string;
  public readonly statusCode?: number;
  public readonly timeout?: number;
  public readonly retryable: boolean;
  public readonly provider: ProviderType;
  
  constructor(options: NetworkErrorOptions) {
    super({
      ...options,
      category: 'network',
      userMessage: options.userMessage || 'Network error occurred. Please check your internet connection and try again.'
    });
    this.name = 'NetworkError';
    this.url = options.url;
    this.statusCode = options.statusCode;
    this.timeout = options.timeout;
    this.retryable = options.retryable !== undefined ? options.retryable : true;
    this.provider = options.provider;
    
    // Add network info to metadata
    if (options.url) this.metadata.url = options.url;
    if (options.statusCode) this.metadata.statusCode = options.statusCode;
    if (options.timeout) this.metadata.timeout = options.timeout;
    this.metadata.provider = options.provider;
    this.metadata.retryable = this.retryable;
  }
}

/**
 * Connection timeout errors
 */
export class ConnectionTimeoutError extends NetworkError {
  constructor(options: NetworkErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Connection timed out${options.url ? ` while connecting to ${options.url}` : ''}. Please check your internet connection and try again.`
    });
    this.name = 'ConnectionTimeoutError';
  }
}

/**
 * Server unreachable errors
 */
export class ServerUnreachableError extends NetworkError {
  constructor(options: NetworkErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `Server unreachable${options.url ? ` at ${options.url}` : ''}. The service may be down or your internet connection may be having issues.`
    });
    this.name = 'ServerUnreachableError';
  }
}

/**
 * HTTP status errors
 */
export class HttpStatusError extends NetworkError {
  constructor(options: NetworkErrorOptions & { statusCode: number }) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `HTTP error ${options.statusCode}${options.url ? ` while connecting to ${options.url}` : ''}. Please try again later.`
    });
    this.name = 'HttpStatusError';
  }
}

/**
 * DNS resolution errors
 */
export class DnsResolutionError extends NetworkError {
  constructor(options: NetworkErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `DNS resolution failed${options.url ? ` for ${options.url}` : ''}. Please check your network configuration and try again.`
    });
    this.name = 'DnsResolutionError';
  }
}

/**
 * SSL/TLS errors
 */
export class TlsError extends NetworkError {
  constructor(options: NetworkErrorOptions) {
    super({
      ...options,
      userMessage: options.userMessage || 
        `SSL/TLS error${options.url ? ` while connecting to ${options.url}` : ''}. This could be due to a network configuration issue.`
    });
    this.name = 'TlsError';
  }
}