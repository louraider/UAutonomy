import java.util.Properties

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// Постійний ключ підпису релізу. Береться зі змінних середовища (CI, секрети
// GitHub) або з app/android/key.properties (локально; файл у .gitignore).
// Сам ключ і паролі ніколи не комітяться. Android приймає оновлення лише з
// тим самим ключем, тому його втрата означає, що люди не зможуть оновитися.
val keyProps = Properties().apply {
    val f = rootProject.file("key.properties")
    if (f.exists()) f.inputStream().use { load(it) }
}

fun signingValue(env: String, prop: String): String? =
    System.getenv(env)?.takeIf { it.isNotBlank() } ?: keyProps.getProperty(prop)

val releaseStoreFile = signingValue("UAUTONOMY_KEYSTORE", "storeFile")
val releaseStorePassword = signingValue("UAUTONOMY_KEYSTORE_PASSWORD", "storePassword")
val releaseKeyAlias = signingValue("UAUTONOMY_KEY_ALIAS", "keyAlias") ?: "uautonomy"
val releaseKeyPassword = signingValue("UAUTONOMY_KEY_PASSWORD", "keyPassword") ?: releaseStorePassword
val hasReleaseKey = releaseStoreFile != null && releaseStorePassword != null

android {
    namespace = "org.uautonomy.uautonomy"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    signingConfigs {
        if (hasReleaseKey) {
            create("release") {
                storeFile = file(releaseStoreFile!!)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    defaultConfig {
        // Остаточний ідентифікатор: після першої роздачі змінити не можна
        // (інакше Android вважатиме це іншим застосунком).
        applicationId = "org.uautonomy.app"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    // Стиснені нативні бібліотеки: APK завантажують з мобільного інтернету й
    // передають «Обміном» між телефонами, тож вага файлу важливіша за кілька
    // десятків МБ на диску після встановлення (Android розпаковує .so).
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }

    buildTypes {
        release {
            signingConfig = if (hasReleaseKey) {
                signingConfigs.getByName("release")
            } else {
                if (System.getenv("UAUTONOMY_REQUIRE_RELEASE_KEY") == "1") {
                    throw GradleException(
                        "Немає ключа підпису релізу: задайте UAUTONOMY_KEYSTORE і UAUTONOMY_KEYSTORE_PASSWORD",
                    )
                }
                logger.warn("УВАГА: реліз підписано debug-ключем (немає постійного ключа). Таку збірку не роздавайте людям.")
                signingConfigs.getByName("debug")
            }
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
