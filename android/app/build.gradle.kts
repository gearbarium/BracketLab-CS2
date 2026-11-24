plugins {
    id("com.android.application") version "8.9.1"
    id("org.jetbrains.kotlin.android") version "2.1.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.20"
    id("org.jetbrains.kotlin.plugin.serialization") version "2.1.20"
    id("com.google.devtools.ksp") version "2.1.20-1.0.32"
}

android {
    namespace = "com.bracketlab"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.bracketlab"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.0"
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    signingConfigs {
        val releaseKey = providers.environmentVariable("BRACKETLAB_KEYSTORE").orNull
        if (releaseKey != null) {
            create("production") {
                storeFile = file(releaseKey)
                storePassword = providers.environmentVariable("BRACKETLAB_STORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("BRACKETLAB_KEY_ALIAS").get()
                keyPassword = providers.environmentVariable("BRACKETLAB_KEY_PASSWORD").get()
            }
        }
    }
    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("production")
        }
    }
}

kotlin { jvmToolchain(17) }

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.05.01"))
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("androidx.room:room-runtime:2.7.1")
    implementation("androidx.room:room-ktx:2.7.1")
    ksp("androidx.room:room-compiler:2.7.1")
    implementation("androidx.datastore:datastore-preferences:1.1.7")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.8.1")
    implementation("io.coil-kt.coil3:coil-compose:3.2.0")
    testImplementation("junit:junit:4.13.2")
}
