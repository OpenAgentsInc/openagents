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
val pushEnabled = file("google-services.json").exists()
if (pushEnabled) apply(plugin = "com.google.gms.google-services")

configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.rustls" && requested.name == "rustls-platform-verifier") {
            useVersion(platformVerifierAndroidVersion)
            because("The Android verifier version must match Cargo.lock")
        }
    }
}

android {
    namespace = appId
    compileSdk = 35
    buildToolsVersion = "35.0.0"
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = JavaVersion.VERSION_17.toString() }

    defaultConfig {
        applicationId = appId
        minSdk = 26
        targetSdk = 35
        versionCode = appBuild
        versionName = appVersion
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        resValue("string", "app_name", appName)
        manifestPlaceholders["openAgentsPushEnabled"] = pushEnabled.toString()
    }

    buildFeatures { buildConfig = true }

    sourceSets["main"].assets.srcDir("../../ios/OpenAgents/Fonts")
    sourceSets["main"].java.srcDir("../platform")
    sourceSets["main"].java.exclude("tests/**")
    sourceSets["main"].java.srcDir("../transcript/src/main/java")
    sourceSets["test"].java.srcDir("../transcript/src/test/java")
    sourceSets["androidTest"].java.srcDir("../transcript/src/androidTest/java")
    sourceSets["androidTest"].java.srcDir("../platform/tests")
    sourceSets["main"].java.srcDir(if (pushEnabled) "src/push/java" else "src/nopush/java")
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
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.camera:camera-camera2:1.5.3")
    implementation("androidx.camera:camera-lifecycle:1.5.3")
    implementation("androidx.camera:camera-view:1.5.3")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("com.google.zxing:core:3.5.4")
    implementation("com.google.mlkit:barcode-scanning:17.3.0")
    implementation("org.rustls:rustls-platform-verifier")
    implementation("com.google.android.play:integrity:1.5.0")
    if (pushEnabled) implementation("com.google.firebase:firebase-messaging:24.1.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
}
