import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uautonomy/screens/chat_screen.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/screens/maps_screen.dart';
import 'package:uautonomy/screens/packs_screen.dart';
import 'package:uautonomy/screens/share_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';
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

  void _goTo(int tab) => setState(() => _tab = tab);

  @override
  void initState() {
    super.initState();
    final dir = widget.dataDir;
    if (dir != null && updateCheckSupported) {
      // Не заважаємо запуску: перевіряємо трохи згодом і лише раз.
      Future.delayed(const Duration(seconds: 5), () async {
        final u = await checkForUpdate(dir);
        if (mounted && u != null) setState(() => _update = u);
      });
    }
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
      MapsScreen(version: libraryVersion, onOpenPacks: () => _goTo(3)),
      PacksScreen(onChanged: () => libraryVersion.value++),
      const ShareScreen(),
    ];
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
                      if (!ok && context.mounted) {
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
