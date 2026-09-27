// The Google services plugin loads only when the app has its Firebase
// configuration, so builds without app/google-services.json never resolve it.
buildscript {
    if (file("app/google-services.json").exists()) {
        repositories { google() }
        dependencies { classpath("com.google.gms:google-services:4.4.2") }
    }
}

plugins {
    id("com.android.application") version "8.13.0" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}

// Keep generated artifacts outside the source checkout, including IDE builds.
val output = providers.gradleProperty("coderOutputDir")
    .orElse(file("../../../../target/coder-android/gradle").absolutePath)
layout.buildDirectory.set(file(output.get()).resolve("root"))
subprojects {
    layout.buildDirectory.set(file(output.get()).resolve(name))
}
