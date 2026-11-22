import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/library.dart';

/// Головний екран: термінові картки за розділами та офлайн-пошук.
class LibraryScreen extends StatefulWidget {
  const LibraryScreen({super.key, required this.version});

  final ValueListenable<int> version;

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  final _query = TextEditingController();
  List<HitView> _hits = const [];
  List<CategoryView> _categories = const [];

  @override
  void initState() {
    super.initState();
    widget.version.addListener(_reload);
    _reload();
  }

  @override
  void dispose() {
    widget.version.removeListener(_reload);
    _query.dispose();
    super.dispose();
  }

  void _reload() {
    setState(() {
      _categories = categories();
      _hits = _query.text.trim().isEmpty
          ? const []
          : search(query: _query.text);
    });
  }

  void _search(String q) {
    setState(() => _hits = q.trim().isEmpty ? const [] : search(query: q));
  }

  void _open(HitView h) {
    if (h.kind == 'card') {
      final c = card(id: h.key.substring('card:'.length));
      if (c != null) openCard(context, c);
    } else {
      Navigator.of(
        context,
      ).push(MaterialPageRoute<void>(builder: (_) => DocScreen(docKey: h.key)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final searching = _query.text.trim().isNotEmpty;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
          child: SearchBar(
            controller: _query,
            hintText: 'Пошук: кровотеча, вода, укриття…',
            leading: const Icon(Icons.search),
            onChanged: _search,
            trailing: [
              if (searching)
                IconButton(
                  icon: const Icon(Icons.clear),
                  tooltip: 'Очистити',
                  onPressed: () {
                    _query.clear();
                    _search('');
                  },
                ),
            ],
          ),
        ),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 16, vertical: 4),
          child: Text(
            emergencyNote,
            style: TextStyle(fontWeight: FontWeight.w600),
          ),
        ),
        Expanded(
          child: searching
              ? (_hits.isEmpty
                    ? const Center(child: Text('Нічого не знайдено'))
                    : ListView(
                        children: [
                          for (final h in _hits)
                            ListTile(
                              leading: Icon(switch (h.kind) {
                                'card' => Icons.bolt,
                                'article' => Icons.article,
                                _ => Icons.local_library,
                              }),
                              title: Text(h.title),
                              subtitle: Text(
                                [
                                  if (h.snippet.isNotEmpty) h.snippet,
                                  h.source,
                                ].join('\n'),
                                maxLines: 3,
                                overflow: TextOverflow.ellipsis,
                              ),
                              onTap: () => _open(h),
                            ),
                        ],
                      ))
              : ListView(
                  children: [
                    for (final cat in _categories)
                      ExpansionTile(
                        title: Text(cat.title),
                        subtitle: Text('${cat.cards.length} карток'),
                        children: [
                          for (final c in cat.cards)
                            ListTile(
                              title: Text(c.title),
                              subtitle: c.summary == null
                                  ? null
                                  : Text(
                                      c.summary!,
                                      maxLines: 2,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                              onTap: () => openCard(context, c),
                            ),
                        ],
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}

void openCard(BuildContext context, CardView c) {
  Navigator.of(context)
      .push(MaterialPageRoute<void>(builder: (_) => CardScreen(card: c)));
}

class CardScreen extends StatelessWidget {
  const CardScreen({super.key, required this.card});

  final CardView card;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    return Scaffold(
      appBar: AppBar(title: Text(card.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (!card.reviewed) ...[
            Align(
              alignment: Alignment.centerLeft,
              child: UnreviewedBadge(who: card.reviewRequired),
            ),
            const SizedBox(height: 12),
          ],
          if (card.summary != null) ...[
            Text(card.summary!, style: t.titleMedium),
            const SizedBox(height: 12),
          ],
          for (final (i, s) in card.steps.indexed)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 6),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  CircleAvatar(radius: 14, child: Text('${i + 1}')),
                  const SizedBox(width: 12),
                  Expanded(child: Text(s, style: t.bodyLarge)),
                ],
              ),
            ),
          if (card.warnings.isNotEmpty) ...[
            const SizedBox(height: 16),
            Text('Чого не робити', style: t.titleMedium),
            for (final w in card.warnings)
              ListTile(
                dense: true,
                leading: const Icon(Icons.block),
                title: Text(w),
              ),
          ],
          const SizedBox(height: 16),
          Text('Джерело: ${card.source}', style: t.bodySmall),
          const SizedBox(height: 8),
          Text(emergencyNote, style: t.bodySmall),
        ],
      ),
    );
  }
}

/// Стаття з пакета або сторінка довідника ZIM (простим текстом).
class DocScreen extends StatelessWidget {
  const DocScreen({super.key, required this.docKey});

  final String docKey;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<DocView?>(
      future: document(key: docKey),
      builder: (context, snap) {
        final d = snap.data;
        return Scaffold(
          appBar: AppBar(title: Text(d?.title ?? '')),
          body: snap.connectionState != ConnectionState.done
              ? const Center(child: CircularProgressIndicator())
              : d == null
              ? const Center(child: Text('Не вдалося відкрити'))
              : ListView(
                  padding: const EdgeInsets.all(16),
                  children: [
                    if (!d.reviewed) ...[
                      const Align(
                        alignment: Alignment.centerLeft,
                        child: UnreviewedBadge(),
                      ),
                      const SizedBox(height: 12),
                    ],
                    SelectableText(d.body),
                    const SizedBox(height: 16),
                    Text(
                      'Джерело: ${d.source}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                ),
        );
      },
    );
  }
}
