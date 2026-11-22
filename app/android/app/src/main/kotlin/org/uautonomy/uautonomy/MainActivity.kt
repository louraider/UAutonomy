package org.uautonomy.uautonomy

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
                    else -> result.notImplemented()
                }
            }
    }
}
