// Resolve Firebase tooling only for a checkout with its local app config.
buildscript {
    if (file("app/google-services.json").exists()) {
        repositories { google() }
        dependencies { classpath("com.google.gms:google-services:4.4.2") }
    }
}

plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}
