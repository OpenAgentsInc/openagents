//! Simple WASI test binary for OANIX
//!
//! This program demonstrates basic WASI operations:
//! - Print to stdout
//! - Read environment variables
//! - Read and write files
//! - List directories

use std::env;
use std::fs;
use std::io::{Read, Write};

fn main() {
    println!("=== OANIX WASI Test ===");
    println!();

    // Print arguments
    println!("Arguments:");
    for (i, arg) in env::args().enumerate() {
        println!("  argv[{}] = {}", i, arg);
    }
    println!();

    // Print environment variables
    println!("Environment:");
    for (key, value) in env::vars() {
        println!("  {} = {}", key, value);
    }
    println!();

    // Try to read a file from /workspace
    println!("Reading /workspace/hello.txt:");
    match fs::read_to_string("/workspace/hello.txt") {
        Ok(content) => println!("  Content: {}", content.trim()),
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    // List /workspace directory
    println!("Listing /workspace:");
    match fs::read_dir("/workspace") {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(e) => {
                        let metadata = e.metadata().ok();
                        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let kind = if is_dir { "dir " } else { "file" };
                        println!("  {} {:>8} {}", kind, size, e.file_name().to_string_lossy());
                    }
                    Err(e) => println!("  Error: {}", e),
                }
            }
        }
        Err(e) => println!("  Error listing directory: {}", e),
    }
    println!();

    // Write a file to /tmp
    println!("Writing /tmp/output.txt:");
    match fs::write("/tmp/output.txt", "Hello from WASI!\nWritten by hello-wasi.\n") {
        Ok(()) => println!("  Success!"),
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    // Create a directory in /tmp
    println!("Creating /tmp/test-dir:");
    match fs::create_dir("/tmp/test-dir") {
        Ok(()) => println!("  Success!"),
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    // Write another file
    println!("Writing /tmp/test-dir/nested.txt:");
    match fs::write("/tmp/test-dir/nested.txt", "Nested file content\n") {
        Ok(()) => println!("  Success!"),
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    // List /tmp to verify
    println!("Listing /tmp:");
    match fs::read_dir("/tmp") {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(e) => {
                        let metadata = e.metadata().ok();
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let kind = if is_dir { "dir " } else { "file" };
                        println!("  {} {}", kind, e.file_name().to_string_lossy());
                    }
                    Err(e) => println!("  Error: {}", e),
                }
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    println!();

    println!("=== Test Complete ===");
}
