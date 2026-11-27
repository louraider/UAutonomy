import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uautonomy/distribution.dart';
import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/screens/chat_screen.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/screens/maps_screen.dart';
import 'package:uautonomy/screens/packs_screen.dart';
import 'package:uautonomy/screens/settings_screen.dart';
import 'package:uautonomy/screens/share_screen.dart';
import 'package:uautonomy/screens/welcome_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/api/packs.dart' show installedPacks;
import 'package:uautonomy/src/rust/frb_generated.dart';
import 'package:uautonomy/stats/stats.dart';
import 'package:uautonomy/theme.dart';
import 'package:uautonomy/update/update_check.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final dir = await getApplicationSupportDirectory();
  if (Platform.isIOS) await _excludeFromBackup(dir.path);
  String? error;
  try {
    await openLibrary(dataDir: dir.path);
  } catch (e) {
    error = '$e';
  }
  await Stats.instance.init(dir.path);
  track('app_open', {'dist': distChannel});
  runApp(UAutonomyApp(startupError: error, dataDir: dir.path));
}

/// На iPhone тека застосунку потрапляє в резервну копію iCloud, а там
/// гігабайти карт, енциклопедій і моделей, які можна завантажити знову.
/// Apple вимагає таке виключати з копії (iCloud Data Storage Guidelines),
/// інакше App Store відхиляє застосунок, а користувачеві забивається iCloud.
Future<void> _excludeFromBackup(String path) async {
  try {
    await const MethodChannel('org.uautonomy/app')
        .invokeMethod<void>('excludeFromBackup', {'path': path});
  } catch (_) {}
}

class UAutonomyApp extends StatelessWidget {
  const UAutonomyApp({super.key, this.startupError, this.dataDir});

  final String? startupError;
  final String? dataDir;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Автономія',
      debugShowCheckedModeBanner: false,
      theme: buildTheme(Brightness.light),
      darkTheme: buildTheme(Brightness.dark),
      home: HomeScreen(startupError: startupError, dataDir: dataDir),
    );
  }
}

/// Сповіщення, що вміст бібліотеки змінився (встановили чи видалили пакет).
final libraryVersion = ValueNotifier<int>(0);

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key, this.startupError, this.dataDir});

  final String? startupError;

  /// Тека даних; потрібна, щоб запам'ятати відкладене оновлення.
  final String? dataDir;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _tab = 0;
  UpdateInfo? _update;
  late final DownloadPrefs? _prefs = widget.dataDir == null
      ? null
      : DownloadPrefs(widget.dataDir!);
  final _downloads = DownloadManager.instance;
  bool _welcomePending = false;

  static const _tabNames = ['library', 'assistant', 'maps', 'packs', 'share'];

  void _goTo(int tab) {
    if (tab != _tab) track('tab_open', {'tab': _tabNames[tab]});
    setState(() => _tab = tab);
  }

  @override
  void initState() {
    super.initState();
    _downloads.onInstalled = () => libraryVersion.value++;
    // Перший запуск: одразу пропонуємо завантажити карту, енциклопедію й
    // модель. Тести застосунку (без dataDir) цей екран не бачать.
    final prefs = _prefs;
    if (prefs != null && !prefs.welcomeDone && widget.startupError == null) {
      bool fresh;
      try {
        fresh = installedPacks().isEmpty;
      } catch (_) {
        fresh = false;
      }
      if (fresh) {
        _welcomePending = true;
      } else {
        prefs.markWelcomeDone();
      }
    }
    // Перший запуск: спершу питаємо згоду на статистику (раз), потім
    // пропонуємо завантаження.
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      await askStatsConsent(context);
      if (_welcomePending && mounted) _showWelcome();
    });
    final dir = widget.dataDir;
    if (dir != null && updateCheckSupported) {
      // Не заважаємо запуску: перевіряємо трохи згодом і лише раз.
      Future.delayed(const Duration(seconds: 5), () async {
        final u = await checkForUpdate(dir);
        if (mounted && u != null) setState(() => _update = u);
      });
    }
  }

  Future<void> _showWelcome() async {
    final started = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (c) => WelcomeScreen(
          manager: _downloads,
          prefs: _prefs,
          onDone: (s) => Navigator.of(c).pop(s),
        ),
      ),
    );
    _prefs?.markWelcomeDone();
    if (started == true && mounted) _goTo(3);
  }

  void _dismissUpdate() {
    final u = _update;
    final dir = widget.dataDir;
    if (u != null && dir != null) dismissUpdate(dir, u.build);
    setState(() => _update = null);
  }

  @override
  Widget build(BuildContext context) {
    final pages = [
      LibraryScreen(version: libraryVersion),
      ChatScreen(version: libraryVersion, onOpenPacks: () => _goTo(3)),
      MapsScreen(
        version: libraryVersion,
        onOpenPacks: () => _goTo(3),
        prefs: _prefs,
      ),
      PacksScreen(
        onChanged: () => libraryVersion.value++,
        manager: _downloads,
        prefs: _prefs,
      ),
      const ShareScreen(),
    ];
    // «Назад» з іншої вкладки веде на «Довідку», а не закриває застосунок:
    // на Android закриття стирало розмову з помічником.
    return PopScope(
      canPop: _tab == 0,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _goTo(0);
      },
      child: _scaffold(pages),
    );
  }

  Widget _scaffold(List<Widget> pages) {
    return Scaffold(
      body: SafeArea(
        child: Column(
          children: [
            if (widget.startupError != null)
              MaterialBanner(
                content: Text(
                  'Не вдалося відкрити бібліотеку: ${widget.startupError}',
                ),
                actions: const [SizedBox.shrink()],
              ),
            if (_update case final u?)
              MaterialBanner(
                leading: const Icon(Icons.system_update),
                content: Text(
                  u.date.isEmpty
                      ? 'Вийшла нова версія «Автономії».'
                      : 'Вийшла нова версія «Автономії» від ${u.date}.',
                ),
                actions: [
                  TextButton(
                    onPressed: _dismissUpdate,
                    child: const Text('Не зараз'),
                  ),
                  FilledButton(
                    onPressed: () async {
                      final ok = await openDownload(u.url);
                      if (!ok && mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text('Завантажте з ${u.url}')),
                        );
                      }
                    },
                    child: const Text('Завантажити'),
                  ),
                ],
              ),
            Expanded(
              child: IndexedStack(index: _tab, children: pages),
            ),
          ],
        ),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tab,
        onDestinationSelected: _goTo,
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.menu_book_outlined),
            selectedIcon: Icon(Icons.menu_book),
            label: 'Довідка',
          ),
          NavigationDestination(
            icon: Icon(Icons.forum_outlined),
            selectedIcon: Icon(Icons.forum),
            label: 'Помічник',
          ),
          NavigationDestination(
            icon: Icon(Icons.map_outlined),
            selectedIcon: Icon(Icons.map),
            label: 'Карти',
          ),
          NavigationDestination(
            icon: Icon(Icons.download_outlined),
            selectedIcon: Icon(Icons.download),
            label: 'Пакети',
          ),
          NavigationDestination(
            icon: Icon(Icons.wifi_tethering),
            label: 'Обмін',
          ),
        ],
      ),
    );
  }
}
