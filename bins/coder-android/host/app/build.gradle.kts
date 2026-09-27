plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val coderAbi = providers.gradleProperty("coderAbi").orElse("arm64-v8a").get()
require(coderAbi in setOf("arm64-v8a", "x86_64")) { "Unsupported Coder Android ABI" }
val nativeDirectory = providers.gradleProperty("coderNativeDir")
    .orElse(rootProject.file("../../../../target/coder-android/jniLibs").absolutePath)

android {
    namespace = "com.openagents.coder"
    compileSdk = 35
    ndkVersion = "27.1.12297006"

    defaultConfig {
        applicationId = "com.openagents.coder"
        minSdk = 26
        targetSdk = 35
        versionCode = 4
        versionName = "0.5.0"
        ndk { abiFilters += coderAbi }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { buildConfig = true }
    sourceSets.getByName("main").jniLibs.srcDir(nativeDirectory)
    packaging { jniLibs.useLegacyPackaging = false }
    buildTypes {
        debug { isJniDebuggable = true }
        release { isMinifyEnabled = false }
    }
    testOptions { animationsDisabled = true }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.camera:camera-core:1.4.2")
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("androidx.camera:camera-view:1.4.2")
    implementation("com.google.zxing:core:3.5.3")
    implementation("io.noties.markwon:core:4.6.2")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:core:1.6.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}
