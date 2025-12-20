//! Unit tests for SecureStore encryption module

use compute::storage::SecureStore;
use tempfile::TempDir;

const TEST_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const TEST_PASSWORD: &str = "strong_password_123";

#[tokio::test]
async fn test_store_and_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    // Store mnemonic
    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Load it back
    let loaded = store.load(TEST_PASSWORD).await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_wrong_password() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Try to load with wrong password
    let result = store.load("wrong_password").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_not_found() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.enc");
    let store = SecureStore::new(path);

    let result = store.load(TEST_PASSWORD).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_exists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    assert!(!store.exists().await);

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    assert!(store.exists().await);
}

#[tokio::test]
async fn test_delete() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();
    assert!(store.exists().await);

    store.delete().await.unwrap();
    assert!(!store.exists().await);
}

#[tokio::test]
async fn test_change_password() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Change password
    let new_password = "new_password_456";
    store.change_password(TEST_PASSWORD, new_password).await.unwrap();

    // Old password should not work
    assert!(store.load(TEST_PASSWORD).await.is_err());

    // New password should work
    let loaded = store.load(new_password).await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_change_password_wrong_old() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Try to change with wrong old password
    let result = store.change_password("wrong", "new").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_different_passwords_different_ciphertexts() {
    let dir = TempDir::new().unwrap();
    let path1 = dir.path().join("test1.enc");
    let path2 = dir.path().join("test2.enc");
    let store1 = SecureStore::new(path1.clone());
    let store2 = SecureStore::new(path2.clone());

    store1.store(TEST_MNEMONIC, "password1").await.unwrap();
    store2.store(TEST_MNEMONIC, "password2").await.unwrap();

    // Different passwords should produce different ciphertexts
    let file1 = tokio::fs::read_to_string(&path1).await.unwrap();
    let file2 = tokio::fs::read_to_string(&path2).await.unwrap();
    assert_ne!(file1, file2);
}

#[tokio::test]
async fn test_same_password_different_salts() {
    let dir = TempDir::new().unwrap();
    let path1 = dir.path().join("test1.enc");
    let path2 = dir.path().join("test2.enc");
    let store1 = SecureStore::new(path1.clone());
    let store2 = SecureStore::new(path2.clone());

    store1.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();
    store2.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Same password but different salts should produce different ciphertexts
    let file1 = tokio::fs::read_to_string(&path1).await.unwrap();
    let file2 = tokio::fs::read_to_string(&path2).await.unwrap();
    assert_ne!(file1, file2);

    // But both should decrypt correctly
    assert_eq!(store1.load(TEST_PASSWORD).await.unwrap(), TEST_MNEMONIC);
    assert_eq!(store2.load(TEST_PASSWORD).await.unwrap(), TEST_MNEMONIC);
}

#[tokio::test]
async fn test_plaintext_storage() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    assert!(!store.plaintext_exists().await);

    store.store_plaintext(TEST_MNEMONIC).await.unwrap();
    assert!(store.plaintext_exists().await);

    let loaded = store.load_plaintext().await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_delete_plaintext() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store_plaintext(TEST_MNEMONIC).await.unwrap();
    assert!(store.plaintext_exists().await);

    store.delete_plaintext().await.unwrap();
    assert!(!store.plaintext_exists().await);
}

#[tokio::test]
async fn test_plaintext_not_found() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    let result = store.load_plaintext().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_password() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    // Empty password should work (though not recommended)
    store.store(TEST_MNEMONIC, "").await.unwrap();
    let loaded = store.load("").await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_long_password() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    let long_password = "a".repeat(1000);
    store.store(TEST_MNEMONIC, &long_password).await.unwrap();
    let loaded = store.load(&long_password).await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_unicode_password() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    let unicode_password = "пароль🔐密码";
    store.store(TEST_MNEMONIC, unicode_password).await.unwrap();
    let loaded = store.load(unicode_password).await.unwrap();
    assert_eq!(loaded, TEST_MNEMONIC);
}

#[tokio::test]
async fn test_long_mnemonic() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    // 24-word mnemonic
    let long_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    store.store(long_mnemonic, TEST_PASSWORD).await.unwrap();
    let loaded = store.load(TEST_PASSWORD).await.unwrap();
    assert_eq!(loaded, long_mnemonic);
}

#[tokio::test]
async fn test_whitespace_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    let mnemonic_with_spaces = "  abandon  abandon  abandon  ";
    store.store(mnemonic_with_spaces, TEST_PASSWORD).await.unwrap();
    let loaded = store.load(TEST_PASSWORD).await.unwrap();
    assert_eq!(loaded, mnemonic_with_spaces);
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.enc");
    let store = SecureStore::new(path);

    // Deleting nonexistent file should not error
    let result = store.delete().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_default_path() {
    let path = SecureStore::default_path();
    assert!(path.to_string_lossy().contains("openagents"));
    assert!(path.to_string_lossy().contains("compute"));
    assert!(path.to_string_lossy().contains("identity.enc"));
}

#[tokio::test]
async fn test_with_default_path() {
    let _store = SecureStore::with_default_path();
    // Just verify it creates without error
}

#[tokio::test]
async fn test_directory_creation() {
    let dir = TempDir::new().unwrap();
    let nested_path = dir.path().join("deeply").join("nested").join("test.enc");
    let store = SecureStore::new(nested_path.clone());

    // Parent directories should be created automatically
    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();
    assert!(nested_path.exists());
}

#[tokio::test]
async fn test_overwrite_existing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path);

    store.store("first mnemonic", TEST_PASSWORD).await.unwrap();
    store.store("second mnemonic", TEST_PASSWORD).await.unwrap();

    let loaded = store.load(TEST_PASSWORD).await.unwrap();
    assert_eq!(loaded, "second mnemonic");
}

#[tokio::test]
async fn test_concurrent_operations() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path.clone());

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Multiple concurrent reads should work
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let store_clone = SecureStore::new(path.clone());
            let password = TEST_PASSWORD.to_string();
            tokio::spawn(async move {
                store_clone.load(&password).await.unwrap()
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert_eq!(result, TEST_MNEMONIC);
    }
}

#[tokio::test]
async fn test_json_format() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path.clone());

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    // Check that file contains expected JSON fields
    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(contents.contains("ciphertext"));
    assert!(contents.contains("nonce"));
    assert!(contents.contains("salt"));
    assert!(contents.contains("version"));
}

#[tokio::test]
async fn test_version_field() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.enc");
    let store = SecureStore::new(path.clone());

    store.store(TEST_MNEMONIC, TEST_PASSWORD).await.unwrap();

    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    let data: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(data["version"], 1);
}
