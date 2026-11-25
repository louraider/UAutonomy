import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    guard let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "UAutonomyApp") else {
      return
    }
    // Той самий канал, що й на Android (MainActivity.kt), зі своїми методами.
    let channel = FlutterMethodChannel(
      name: "org.uautonomy/app", binaryMessenger: registrar.messenger())
    channel.setMethodCallHandler { call, result in
      let args = call.arguments as? [String: Any]
      switch call.method {
      // Пакети й моделі (гігабайти) не мають потрапляти в копію iCloud:
      // їх можна завантажити знову. Прапорець на теці діє на весь її вміст.
      case "excludeFromBackup":
        guard let path = args?["path"] as? String else {
          result(FlutterError(code: "args", message: "path", details: nil))
          return
        }
        do {
          try FileManager.default.createDirectory(
            atPath: path, withIntermediateDirectories: true)
          var url = URL(fileURLWithPath: path, isDirectory: true)
          var values = URLResourceValues()
          values.isExcludedFromBackup = true
          try url.setResourceValues(values)
          result(nil)
        } catch {
          result(FlutterError(code: "backup", message: "\(error)", details: nil))
        }
      // Поки працює обмін, екран не гасне: інакше iOS призупиняє
      // застосунок і сервер перестає відповідати.
      case "keepAwake":
        UIApplication.shared.isIdleTimerDisabled = (args?["on"] as? Bool) ?? false
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
  }
}
