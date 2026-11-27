import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/screens/settings_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/stats/stats.dart';
import 'package:uautonomy/theme.dart';
import 'package:uautonomy/widgets/brand.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

/// Головний екран: термінові картки за розділами та офлайн-пошук.
class LibraryScreen extends StatefulWidget {
  const LibraryScreen({
    super.key,
    required this.version,
    this.loadCategories = categories,
  });

  final ValueListenable<int> version;

  /// Джерело розділів; у тестах вигляду підставляють приклад.
  final List<CategoryView> Function() loadCategories;

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
    _searchStat?.cancel();
    super.dispose();
  }

  void _reload() {
    setState(() {
      _categories = widget.loadCategories();
      _hits = _query.text.trim().isEmpty
          ? const []
          : search(query: _query.text);
    });
  }

  Timer? _searchStat;

  void _search(String q) {
    setState(() => _hits = q.trim().isEmpty ? const [] : search(query: q));
    // Одна подія на пошук, коли людина перестала друкувати. Лише чи
    // знайшлося щось, без самого тексту.
    _searchStat?.cancel();
    if (q.trim().length >= 3) {
      _searchStat = Timer(const Duration(seconds: 3), () {
        if (mounted) track('search', {'found': _hits.isEmpty ? 'no' : 'yes'});
      });
    }
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
    final b = BrandColors.of(context);
    return ContentWidth(
      child: CustomScrollView(
        slivers: [
          SliverToBoxAdapter(
            child: TabHeader(
              title: 'Довідка',
              showMark: true,
              trailing: [
                IconButton(
                  icon: const Icon(Icons.settings_outlined),
                  tooltip: 'Налаштування і відгук',
                  onPressed: () => Navigator.of(context).push(
                    MaterialPageRoute<void>(
                      builder: (_) => const SettingsScreen(),
                    ),
                  ),
                ),
              ],
            ),
          ),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
            sliver: SliverList.list(
              children: [
                SearchBar(
                  controller: _query,
                  hintText: 'Пошук: кровотеча, вода, укриття…',
                  leading: Icon(Icons.search, color: b.muted),
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
                const SizedBox(height: 12),
                const SosBar(),
                const SizedBox(height: 20),
              ],
            ),
          ),
          if (searching)
            _hits.isEmpty
                ? const SliverToBoxAdapter(
                    child: Padding(
                      padding: EdgeInsets.all(24),
                      child: Center(child: Text('Нічого не знайдено')),
                    ),
                  )
                : SliverList.list(
                    children: [
                      const _SectionLabel('Знайдено'),
                      Card(
                        child: Column(
                          children: [
                            for (final (i, h) in _hits.indexed) ...[
                              if (i > 0) const Divider(indent: 56),
                              ListTile(
                                leading: Icon(switch (h.kind) {
                                  'card' => Icons.bolt,
                                  'article' => Icons.article_outlined,
                                  _ => Icons.local_library_outlined,
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
                          ],
                        ),
                      ),
                      const SizedBox(height: 16),
                    ],
                  )
          else ...[
            const SliverToBoxAdapter(child: _SectionLabel('Що робити зараз')),
            SliverPadding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
              sliver: SliverGrid.builder(
                gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                  maxCrossAxisExtent: 200,
                  mainAxisExtent: 136,
                  mainAxisSpacing: 12,
                  crossAxisSpacing: 12,
                ),
                itemCount: _categories.length,
                itemBuilder: (context, i) =>
                    _CategoryTile(category: _categories[i]),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// Іконка розділу карток за його ідентифікатором.
IconData categoryIcon(String slug) => switch (slug) {
  'first_aid' => Icons.medical_services_outlined,
  'civil_protection' => Icons.shield_outlined,
  'mine_safety' => Icons.warning_amber_rounded,
  'household' => Icons.home_outlined,
  _ => Icons.bolt,
};

String cardsCount(int n) {
  final m10 = n % 10, m100 = n % 100;
  final word = m10 == 1 && m100 != 11
      ? 'картка'
      : (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14))
      ? 'картки'
      : 'карток';
  return '$n $word';
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
    child: Text(text, style: Theme.of(context).textTheme.titleMedium),
  );
}

class _CategoryTile extends StatelessWidget {
  const _CategoryTile({required this.category});

  final CategoryView category;

  @override
  Widget build(BuildContext context) {
    final b = BrandColors.of(context);
    final t = Theme.of(context).textTheme;
    return Material(
      color: b.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(color: b.line),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () {
          track('category_open', {'category': category.slug});
          Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => CategoryScreen(category: category),
            ),
          );
        },
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: b.blue,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Icon(
                  categoryIcon(category.slug),
                  color: b.onBlue,
                  size: 22,
                ),
              ),
              const Spacer(),
              Text(
                category.title,
                style: t.titleMedium?.copyWith(fontSize: 16, height: 1.2),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
              ),
              const SizedBox(height: 4),
              Text(
                cardsCount(category.cards.length).toUpperCase(),
                style: monoLabel(context),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Усі картки одного розділу.
class CategoryScreen extends StatelessWidget {
  const CategoryScreen({super.key, required this.category});

  final CategoryView category;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(category.title)),
      body: ContentWidth(
        child: ListView(
          padding: const EdgeInsets.only(bottom: 24),
          children: [
            Card(
              child: Column(
                children: [
                  for (final (i, c) in category.cards.indexed) ...[
                    if (i > 0) const Divider(indent: 16),
                    ListTile(
                      title: Text(c.title),
                      subtitle: c.summary == null
                          ? null
                          : Text(
                              c.summary!,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                            ),
                      trailing: const Icon(Icons.chevron_right),
                      onTap: () => openCard(context, c),
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

void openCard(BuildContext context, CardView c) {
  track('card_open', {'card': c.id, 'category': c.category});
  Navigator.of(context)
      .push(MaterialPageRoute<void>(builder: (_) => CardScreen(card: c)));
}

class CardScreen extends StatelessWidget {
  const CardScreen({super.key, required this.card});

  final CardView card;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final b = BrandColors.of(context);
    return Scaffold(
      appBar: AppBar(),
      body: ContentWidth(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 32),
          children: [
            Text(card.title, style: t.headlineSmall),
            const SizedBox(height: 12),
            if (!card.reviewed) ...[
              Align(
                alignment: Alignment.centerLeft,
                child: UnreviewedBadge(who: card.reviewRequired),
              ),
              const SizedBox(height: 12),
            ],
            if (card.summary != null) ...[
              Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: b.tint,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Text(
                  card.summary!,
                  style: t.bodyLarge?.copyWith(fontWeight: FontWeight.w600),
                ),
              ),
              const SizedBox(height: 16),
            ],
            for (final (i, s) in card.steps.indexed)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 8),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 32,
                      height: 32,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: b.blue,
                        shape: BoxShape.circle,
                      ),
                      child: Text(
                        '${i + 1}',
                        style: TextStyle(
                          fontFamily: displayFont,
                          fontWeight: FontWeight.w800,
                          fontSize: 15,
                          color: b.onBlue,
                        ),
                      ),
                    ),
                    const SizedBox(width: 14),
                    Expanded(
                      child: Padding(
                        padding: const EdgeInsets.only(top: 3),
                        child: Text(s, style: t.bodyLarge),
                      ),
                    ),
                  ],
                ),
              ),
            if (card.warnings.isNotEmpty) ...[
              const SizedBox(height: 16),
              Container(
                padding: const EdgeInsets.fromLTRB(14, 14, 14, 6),
                decoration: BoxDecoration(
                  color: b.dangerTint,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Чого не робити',
                      style: t.titleMedium?.copyWith(color: b.danger),
                    ),
                    const SizedBox(height: 6),
                    for (final w in card.warnings)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Icon(Icons.block, size: 20, color: b.danger),
                            const SizedBox(width: 10),
                            Expanded(child: Text(w, style: t.bodyMedium)),
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ],
            if (card.sourceQuote != null) ...[
              const SizedBox(height: 16),
              _SourceQuote(card: card),
            ],
            const SizedBox(height: 20),
            Text('Джерело: ${card.source}', style: t.bodySmall),
            if (card.sourceUrl != null)
              SelectableText('Оригінал: ${card.sourceUrl}', style: t.bodySmall),
            const SizedBox(height: 16),
            const SosBar(),
          ],
        ),
      ),
    );
  }
}

/// Дослівний текст офіційного джерела під кроками картки. Кроки вище —
/// переказ простими словами; якщо вони розходяться, правильний цей текст.
class _SourceQuote extends StatelessWidget {
  const _SourceQuote({required this.card});

  final CardView card;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final medical =
        card.category == 'first_aid' || card.reviewRequired == 'medic';
    return Card(
      margin: EdgeInsets.zero,
      child: ExpansionTile(
        initiallyExpanded: true,
        shape: const Border(),
        title: Text(
          medical ? 'Дослівно з наказу МОЗ № 441' : 'Дослівно з джерела',
          style: t.titleMedium,
        ),
        subtitle: const Text(
          'Офіційний текст. Кроки вище — переказ простими словами; '
          'якщо вони розходяться, правильний цей текст.',
        ),
        childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (!card.reviewed) ...[
            UnreviewedBadge(who: card.reviewRequired),
            const SizedBox(height: 8),
          ],
          SelectableText(card.sourceQuote!, style: t.bodyMedium),
          if (card.sourceUrl != null) ...[
            const SizedBox(height: 8),
            SelectableText(card.sourceUrl!, style: t.bodySmall),
          ],
        ],
      ),
    );
  }
}

/// Відкриває посилання зі статті: картку чи іншу сторінку довідника.
void openLink(BuildContext context, String target) {
  if (target.startsWith('card:')) {
    final c = card(id: target.substring('card:'.length));
    if (c != null) openCard(context, c);
  } else if (target.startsWith('zim:') || target.startsWith('article:')) {
    Navigator.of(
      context,
    ).push(MaterialPageRoute<void>(builder: (_) => DocScreen(docKey: target)));
  }
}

/// Стаття з пакета або сторінка довідника ZIM (Markdown).
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
              : ContentWidth(
                  child: ListView(
                    padding: const EdgeInsets.fromLTRB(16, 0, 16, 32),
                    children: [
                      if (!d.reviewed) ...[
                        Align(
                          alignment: Alignment.centerLeft,
                          child: UnreviewedBadge(who: d.reviewRequired),
                        ),
                        const SizedBox(height: 12),
                      ],
                      MarkdownView(
                        data: d.body,
                        onLink: (target) => openLink(context, target),
                      ),
                      const SizedBox(height: 16),
                      Text(
                        'Джерело: ${d.source}',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      if (d.reviewRequired == 'medic') ...[
                        const SizedBox(height: 8),
                        Text(
                          emergencyNote,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ],
                    ],
                  ),
                ),
        );
      },
    );
  }
}
