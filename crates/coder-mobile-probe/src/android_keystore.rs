//! Exercise an app-scoped synthetic Android Keystore key without exporting it.

use jni::{Env, jni_sig, jni_str, objects::JObject, objects::JValue};

pub(super) fn probe(env: &mut Env<'_>) -> jni::errors::Result<serde_json::Value> {
    let provider = env.new_string("AndroidKeyStore")?;
    let store = env
        .call_static_method(
            jni_str!("java/security/KeyStore"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;)Ljava/security/KeyStore;"),
            &[JValue::Object(&provider)],
        )?
        .l()?;
    let null = JObject::null();
    env.call_method(
        &store,
        jni_str!("load"),
        jni_sig!("(Ljava/security/KeyStore$LoadStoreParameter;)V"),
        &[JValue::Object(&null)],
    )?;
    let alias = env.new_string("coder-platform-probe-synthetic-v1")?;
    env.call_method(
        &store,
        jni_str!("deleteEntry"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&alias)],
    )?;
    let algorithm = env.new_string("AES")?;
    let generator = env
        .call_static_method(
            jni_str!("javax/crypto/KeyGenerator"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;"),
            &[JValue::Object(&algorithm), JValue::Object(&provider)],
        )?
        .l()?;
    let builder = env.new_object(
        jni_str!("android/security/keystore/KeyGenParameterSpec$Builder"),
        jni_sig!("(Ljava/lang/String;I)V"),
        &[JValue::Object(&alias), JValue::Int(3)],
    )?;
    let modes = env.new_object_array(1, jni_str!("java/lang/String"), &null)?;
    let gcm = env.new_string("GCM")?;
    modes.set_element(env, 0, &gcm)?;
    env.call_method(
        &builder,
        jni_str!("setBlockModes"),
        jni_sig!("([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;"),
        &[JValue::Object(&modes)],
    )?;
    let padding = env.new_object_array(1, jni_str!("java/lang/String"), &null)?;
    let none = env.new_string("NoPadding")?;
    padding.set_element(env, 0, &none)?;
    env.call_method(
        &builder,
        jni_str!("setEncryptionPaddings"),
        jni_sig!("([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;"),
        &[JValue::Object(&padding)],
    )?;
    let specification = env
        .call_method(
            &builder,
            jni_str!("build"),
            jni_sig!("()Landroid/security/keystore/KeyGenParameterSpec;"),
            &[],
        )?
        .l()?;
    env.call_method(
        &generator,
        jni_str!("init"),
        jni_sig!("(Ljava/security/spec/AlgorithmParameterSpec;)V"),
        &[JValue::Object(&specification)],
    )?;
    let key = env
        .call_method(
            &generator,
            jni_str!("generateKey"),
            jni_sig!("()Ljavax/crypto/SecretKey;"),
            &[],
        )?
        .l()?;
    let transform = env.new_string("AES/GCM/NoPadding")?;
    let cipher = env
        .call_static_method(
            jni_str!("javax/crypto/Cipher"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;)Ljavax/crypto/Cipher;"),
            &[JValue::Object(&transform)],
        )?
        .l()?;
    env.call_method(
        &cipher,
        jni_str!("init"),
        jni_sig!("(ILjava/security/Key;)V"),
        &[JValue::Int(1), JValue::Object(&key)],
    )?;
    let marker = b"public-synthetic-keystore-marker";
    let plaintext = env.byte_array_from_slice(marker)?;
    let ciphertext = env
        .call_method(
            &cipher,
            jni_str!("doFinal"),
            jni_sig!("([B)[B"),
            &[JValue::Object(&plaintext)],
        )?
        .l()?;
    let iv = env
        .call_method(&cipher, jni_str!("getIV"), jni_sig!("()[B"), &[])?
        .l()?;
    let parameters = env.new_object(
        jni_str!("javax/crypto/spec/GCMParameterSpec"),
        jni_sig!("(I[B)V"),
        &[JValue::Int(128), JValue::Object(&iv)],
    )?;
    env.call_method(
        &cipher,
        jni_str!("init"),
        jni_sig!("(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V"),
        &[
            JValue::Int(2),
            JValue::Object(&key),
            JValue::Object(&parameters),
        ],
    )?;
    let returned = env
        .call_method(
            &cipher,
            jni_str!("doFinal"),
            jni_sig!("([B)[B"),
            &[JValue::Object(&ciphertext)],
        )?
        .l()?;
    let equal = env
        .call_static_method(
            jni_str!("java/util/Arrays"),
            jni_str!("equals"),
            jni_sig!("([B[B)Z"),
            &[JValue::Object(&plaintext), JValue::Object(&returned)],
        )?
        .z()?;
    env.call_method(
        &store,
        jni_str!("deleteEntry"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&alias)],
    )?;
    let remains = env
        .call_method(
            &store,
            jni_str!("containsAlias"),
            jni_sig!("(Ljava/lang/String;)Z"),
            &[JValue::Object(&alias)],
        )?
        .z()?;
    Ok(
        serde_json::json!({"event":"keystore_probe","synthetic":true,"bytes_equal":equal,"deleted":!remains,"passed":equal && !remains,"hardware_backing":"not_checked","locked_device_test":"not_run"}),
    )
}
