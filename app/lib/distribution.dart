/// Звідки людина отримала застосунок. Задається під час збірки:
///
///   --dart-define=UAUTONOMY_DIST=site     APK і інсталятор із uautonomy.app (типово)
///   --dart-define=UAUTONOMY_DIST=play     Google Play
///   --dart-define=UAUTONOMY_DIST=msstore  Microsoft Store
///
/// У збірках для магазинів застосунок оновлює сам магазин. Google Play
/// забороняє застосункам оновлювати чи роздавати себе повз Play, тому там
/// немає сповіщень про нову версію і «Обмін» не роздає APK.
library;

const String distChannel = String.fromEnvironment(
  'UAUTONOMY_DIST',
  defaultValue: 'site',
);

/// Збірка для Google Play чи Microsoft Store.
const bool isStoreBuild = distChannel == 'play' || distChannel == 'msstore';

/// Адреса для скарг на відповіді ШІ та інших повідомлень від людей.
const String supportEmail = 'support@uautonomy.app';

const String issuesUrl = 'https://github.com/louraider/UAutonomy/issues/new';
