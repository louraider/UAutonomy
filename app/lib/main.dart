import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/src/rust/api/simple.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const UAutonomyApp());
}

class UAutonomyApp extends StatelessWidget {
  const UAutonomyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Автономія',
      theme: ThemeData(colorSchemeSeed: const Color(0xFF0057B7)),
      home: const SearchPage(),
    );
  }
}

class SearchPage extends StatefulWidget {
  const SearchPage({super.key});

  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> {
  List<CardView> _results = const [];

  void _search(String query) {
    setState(() => _results = searchCards(query: query));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Автономія · ядро ${coreVersion()}')),
      body: Column(
        children: [
          const MaterialBanner(
            content: Text(
              'Ранній каркас. Картки демонстраційні й не є медичними '
              'рекомендаціями. У небезпеці телефонуйте 112.',
            ),
            actions: [SizedBox.shrink()],
          ),
          Padding(
            padding: const EdgeInsets.all(16),
            child: TextField(
              decoration: const InputDecoration(
                labelText: 'Пошук (працює без інтернету)',
                border: OutlineInputBorder(),
              ),
              onChanged: _search,
            ),
          ),
          Expanded(
            child: ListView(
              children: [
                for (final card in _results)
                  ListTile(
                    title: Text(card.title),
                    subtitle: Text('${card.steps.join('\n')}\n${card.source}'),
                    isThreeLine: true,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
