plugins { id("com.android.application"); id("org.jetbrains.kotlin.android") }
android {
    namespace = "com.rustyprompter.app"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.rustyprompter.app"
        minSdk = 30
        targetSdk = 35
        versionCode = 3
        versionName = "1.1.0"
        ndk { abiFilters += listOf("armeabi-v7a", "arm64-v8a", "x86_64") }
    }
    signingConfigs { create("localRelease") { storeFile = file("../../.signing/rustyprompter.jks"); storePassword = System.getenv("RUSTYPROMPTER_STORE_PASSWORD") ?: ""; keyAlias = "rustyprompter"; keyPassword = storePassword } }
    buildTypes { release { isMinifyEnabled = false; signingConfig = signingConfigs.getByName("localRelease") } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
    androidResources { noCompress += listOf("wasm", "mdl", "fst", "int", "vec", "ie", "dubm", "mat") }
}
dependencies {
    implementation("com.alphacephei:vosk-android:0.3.75@aar")
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    implementation("org.java-websocket:Java-WebSocket:1.5.7")
}
