import groovy.json.JsonSlurper

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val releaseMetadata = JsonSlurper().parse(file("../../ios/release/version.json")) as Map<*, *>
val appVersion = releaseMetadata["version"] as String
val appBuild = (releaseMetadata["build"] as String).toInt()
val brandSource = file("../../../crates/brand/src/lib.rs").readText()
val appId = Regex("""pub const MOBILE_BUNDLE_ID: &str = "([^"]+)";""")
    .find(brandSource)?.groupValues?.get(1)
    ?: error("Add MOBILE_BUNDLE_ID to apps/openagents/crates/brand/src/lib.rs")
val appName = Regex("""pub const NAME: &str = "([^"]+)";""")
    .find(brandSource)?.groupValues?.get(1)
    ?: error("Could not read the app name from the brand crate")
val cargoTargetDir = System.getenv("CARGO_TARGET_DIR")
    ?: error("Set CARGO_TARGET_DIR to the shared OpenAgents app target")
val cargoLock = file("../../../Cargo.lock").readText()
val platformVerifierAndroidVersion = Regex(
    """name = "rustls-platform-verifier-android"\s+version = "([^"]+)""",
).find(cargoLock)?.groupValues?.get(1)
    ?: error("rustls-platform-verifier-android is missing from apps/openagents/Cargo.lock")

android {
    namespace = appId
    compileSdk = 35

    defaultConfig {
        applicationId = appId
        minSdk = 26
        targetSdk = 35
        versionCode = appBuild
        versionName = appVersion
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        resValue("string", "app_name", appName)
    }

    buildFeatures { buildConfig = true }

    sourceSets["main"].assets.srcDir("../../ios/OpenAgents/Fonts")
    sourceSets["main"].jniLibs.srcDir("$cargoTargetDir/android-core/jniLibs")

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("rustls:rustls-platform-verifier:$platformVerifierAndroidVersion")
}
