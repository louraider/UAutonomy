package org.uautonomy.uautonomy

import android.content.Intent
import android.net.Uri
import android.os.Build
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // Шлях до власного APK, щоб роздати застосунок іншим телефонам по Wi-Fi.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "org.uautonomy/app")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "apkPath" -> result.success(applicationInfo.sourceDir)
                    // APK збирається окремо під кожну архітектуру; щоб назвати
                    // файл для роздачі, дізнаємося, яку саме встановлено.
                    "apkAbi" -> result.success(installedAbi())
                    // Сторінка завантаження нової версії у браузері.
                    "openUrl" -> {
                        val url = call.argument<String>("url")
                        if (url == null || !url.startsWith("https://")) {
                            result.success(false)
                        } else {
                            try {
                                startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                                result.success(true)
                            } catch (e: Exception) {
                                result.success(false)
                            }
                        }
                    }
                    else -> result.notImplemented()
                }
            }
    }

    private fun installedAbi(): String {
        val dir = applicationInfo.nativeLibraryDir?.substringAfterLast('/') ?: ""
        return when (dir) {
            "arm64" -> "arm64-v8a"
            "arm" -> "armeabi-v7a"
            "x86_64" -> "x86_64"
            "x86" -> "x86"
            else -> Build.SUPPORTED_ABIS.firstOrNull() ?: "android"
        }
    }
}
