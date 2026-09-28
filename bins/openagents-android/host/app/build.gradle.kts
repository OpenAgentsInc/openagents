plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val openagentsAbi = providers.gradleProperty("openagentsAbi").orElse("arm64-v8a").get()
require(openagentsAbi in setOf("arm64-v8a", "x86_64")) { "Unsupported OpenAgents Android ABI" }
val nativeDirectory = providers.gradleProperty("openagentsNativeDir")
    .orElse(rootProject.file("../../../../target/openagents-android/jniLibs").absolutePath)

// Release signing reads a keystore the owner keeps outside the checkout.
// Without these properties, the release build is unsigned.
fun signingSetting(name: String) = providers.gradleProperty(name).orNull
val releaseStore = signingSetting("openagentsReleaseStoreFile")

android {
    namespace = "com.openagents.app"
    compileSdk = 35
    ndkVersion = "27.1.12297006"

    defaultConfig {
        applicationId = "com.openagents.app"
        minSdk = 26
        targetSdk = 35
        versionCode = providers.gradleProperty("openagentsVersionCode").orElse("1").get().toInt()
        versionName = "1.0.0"
        ndk { abiFilters += openagentsAbi }
    }

    signingConfigs {
        if (releaseStore != null) create("release") {
            storeFile = file(releaseStore)
            storePassword = signingSetting("openagentsReleaseStorePassword")
            keyAlias = signingSetting("openagentsReleaseKeyAlias")
            keyPassword = signingSetting("openagentsReleaseKeyPassword")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { buildConfig = true }
    sourceSets.getByName("main").jniLibs.srcDir(nativeDirectory)
    // Debug builds carry Rust Native's sample conversation for the
    // `rust_native_fixture` launch extra.
    sourceSets.getByName("debug").assets.srcDir(rootProject.file("../../../crates/rust-native/fixtures"))
    // Uncompressed, page-aligned native libraries (16 KiB pages).
    packaging { jniLibs.useLegacyPackaging = false }
    buildTypes {
        debug { isJniDebuggable = true }
        release {
            isMinifyEnabled = false
            if (releaseStore != null) signingConfig = signingConfigs.getByName("release")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.recyclerview:recyclerview:1.3.2")
    implementation("androidx.camera:camera-core:1.4.2")
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("androidx.camera:camera-view:1.4.2")
    implementation("com.google.zxing:core:3.5.3")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
