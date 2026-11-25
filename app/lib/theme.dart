import 'package:material_ui/material_ui.dart';

/// Фірмовий стиль «Автономії», напрям «Мапа»: яскравий синій, жовта мітка
/// «ви тут» і горизонталі. Усі кольори застосунку беруться звідси.
///
/// Шрифти вбудовані (assets/fonts, ліцензія OFL), бо застосунок працює
/// без інтернету: Wix Madefor Display для заголовків, Wix Madefor Text для
/// тексту, IBM Plex Mono для службових позначок.
class BrandColors extends ThemeExtension<BrandColors> {
  const BrandColors({
    required this.ground,
    required this.surface,
    required this.ink,
    required this.muted,
    required this.line,
    required this.blue,
    required this.onBlue,
    required this.tint,
    required this.marker,
    required this.onMarker,
    required this.danger,
    required this.dangerTint,
    required this.warnTint,
    required this.onWarnTint,
  });

  final Color ground;
  final Color surface;
  final Color ink;
  final Color muted;
  final Color line;
  final Color blue;
  final Color onBlue;
  final Color tint;

  /// Жовта мітка: смуга SOS, головні дії на синьому.
  final Color marker;
  final Color onMarker;
  final Color danger;
  final Color dangerTint;

  /// Позначка «ще не перевірено фахівцем».
  final Color warnTint;
  final Color onWarnTint;

  static const light = BrandColors(
    ground: Color(0xFFEEF2F6),
    surface: Color(0xFFFFFFFF),
    ink: Color(0xFF0D1B2A),
    muted: Color(0xFF4B5A6B),
    line: Color(0xFFD3DBE4),
    blue: Color(0xFF1F4FD1),
    onBlue: Color(0xFFFFFFFF),
    tint: Color(0xFFE2E9FB),
    marker: Color(0xFFFFD028),
    onMarker: Color(0xFF0D1B2A),
    danger: Color(0xFFC42B1C),
    dangerTint: Color(0xFFFBE9E7),
    warnTint: Color(0xFFFFF4C7),
    onWarnTint: Color(0xFF4A3B00),
  );

  static const dark = BrandColors(
    ground: Color(0xFF0A1220),
    surface: Color(0xFF111C2E),
    ink: Color(0xFFE6ECF5),
    muted: Color(0xFF93A3B8),
    line: Color(0xFF22324A),
    blue: Color(0xFF7FA2FF),
    onBlue: Color(0xFF0A1220),
    tint: Color(0xFF18284A),
    marker: Color(0xFFFFD028),
    onMarker: Color(0xFF0D1B2A),
    danger: Color(0xFFFF8A80),
    dangerTint: Color(0xFF3A1A1A),
    warnTint: Color(0xFF3A3110),
    onWarnTint: Color(0xFFFFE08A),
  );

  static BrandColors of(BuildContext context) =>
      Theme.of(context).extension<BrandColors>() ?? light;

  @override
  BrandColors copyWith() => this;

  @override
  BrandColors lerp(BrandColors? other, double t) =>
      other == null || t < .5 ? this : other;
}

const displayFont = 'WixMadeforDisplay';
const textFont = 'WixMadeforText';
const monoFont = 'IBMPlexMono';

/// Службова позначка моноширинним шрифтом: «3 ХВ · ВОДА».
TextStyle monoLabel(BuildContext context, {Color? color}) => TextStyle(
  fontFamily: monoFont,
  fontWeight: FontWeight.w500,
  fontSize: 11.5,
  letterSpacing: .6,
  color: color ?? BrandColors.of(context).muted,
);

ThemeData buildTheme(Brightness brightness) {
  final b = brightness == Brightness.dark
      ? BrandColors.dark
      : BrandColors.light;
  final scheme =
      ColorScheme.fromSeed(
        seedColor: const Color(0xFF1F4FD1),
        brightness: brightness,
      ).copyWith(
        primary: b.blue,
        onPrimary: b.onBlue,
        primaryContainer: b.tint,
        onPrimaryContainer: b.ink,
        secondary: b.blue,
        onSecondary: b.onBlue,
        secondaryContainer: b.tint,
        onSecondaryContainer: b.ink,
        tertiary: b.marker,
        onTertiary: b.onMarker,
        tertiaryContainer: b.warnTint,
        onTertiaryContainer: b.onWarnTint,
        error: b.danger,
        surface: b.ground,
        onSurface: b.ink,
        onSurfaceVariant: b.muted,
        surfaceContainerLowest: b.surface,
        surfaceContainerLow: b.surface,
        surfaceContainer: b.surface,
        surfaceContainerHigh: b.surface,
        surfaceContainerHighest: b.tint,
        outline: b.muted,
        outlineVariant: b.line,
      );

  final base = ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    fontFamily: textFont,
  );
  TextStyle? d(TextStyle? s, FontWeight w, {double? ls}) => s?.copyWith(
    fontFamily: displayFont,
    fontWeight: w,
    letterSpacing: ls,
    color: b.ink,
  );
  final tt = base.textTheme;
  final textTheme = tt.copyWith(
    displaySmall: d(tt.displaySmall, FontWeight.w800, ls: -.5),
    headlineLarge: d(tt.headlineLarge, FontWeight.w800, ls: -.4),
    headlineMedium: d(tt.headlineMedium, FontWeight.w800, ls: -.3),
    headlineSmall: d(tt.headlineSmall, FontWeight.w800, ls: -.2),
    titleLarge: d(tt.titleLarge, FontWeight.w700),
    titleMedium: d(tt.titleMedium, FontWeight.w700),
    titleSmall: tt.titleSmall?.copyWith(
      fontWeight: FontWeight.w600,
      letterSpacing: .1,
    ),
    bodyLarge: tt.bodyLarge?.copyWith(
      fontSize: 17,
      height: 1.5,
      letterSpacing: .1,
    ),
    bodyMedium: tt.bodyMedium?.copyWith(
      fontSize: 15,
      height: 1.45,
      letterSpacing: .1,
    ),
    bodySmall: tt.bodySmall?.copyWith(
      color: b.muted,
      height: 1.4,
      letterSpacing: .1,
    ),
    labelLarge: tt.labelLarge?.copyWith(
      fontWeight: FontWeight.w600,
      letterSpacing: .1,
    ),
  );
  final radius = BorderRadius.circular(14);
  final border = RoundedRectangleBorder(
    borderRadius: radius,
    side: BorderSide(color: b.line),
  );
  const buttonPadding = EdgeInsets.symmetric(horizontal: 20, vertical: 14);
  final buttonShape = WidgetStatePropertyAll(
    RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
  );

  return base.copyWith(
    textTheme: textTheme,
    scaffoldBackgroundColor: b.ground,
    canvasColor: b.ground,
    dividerColor: b.line,
    extensions: [b],
    appBarTheme: AppBarTheme(
      backgroundColor: b.ground,
      foregroundColor: b.ink,
      surfaceTintColor: Colors.transparent,
      scrolledUnderElevation: 0,
      centerTitle: false,
      titleTextStyle: textTheme.titleLarge,
    ),
    cardTheme: CardThemeData(
      color: b.surface,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      shape: border,
      clipBehavior: Clip.antiAlias,
    ),
    dividerTheme: DividerThemeData(color: b.line, space: 1, thickness: 1),
    listTileTheme: ListTileThemeData(
      iconColor: b.blue,
      titleTextStyle: textTheme.titleSmall?.copyWith(
        fontSize: 16,
        color: b.ink,
      ),
      subtitleTextStyle: textTheme.bodySmall?.copyWith(fontSize: 13.5),
    ),
    expansionTileTheme: ExpansionTileThemeData(
      iconColor: b.blue,
      collapsedIconColor: b.muted,
      shape: const Border(),
      collapsedShape: const Border(),
    ),
    searchBarTheme: SearchBarThemeData(
      elevation: const WidgetStatePropertyAll(0),
      backgroundColor: WidgetStatePropertyAll(b.surface),
      surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
      side: WidgetStatePropertyAll(BorderSide(color: b.line)),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
      constraints: const BoxConstraints(minHeight: 50),
      padding: const WidgetStatePropertyAll(
        EdgeInsets.symmetric(horizontal: 12),
      ),
      hintStyle: WidgetStatePropertyAll(
        TextStyle(color: b.muted, fontSize: 15),
      ),
      textStyle: WidgetStatePropertyAll(TextStyle(color: b.ink, fontSize: 16)),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: b.surface,
      hintStyle: TextStyle(color: b.muted),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: BorderSide(color: b.line),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: BorderSide(color: b.line),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: BorderSide(color: b.blue, width: 2),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: ButtonStyle(
        padding: const WidgetStatePropertyAll(buttonPadding),
        shape: buttonShape,
        textStyle: const WidgetStatePropertyAll(
          TextStyle(
            fontFamily: textFont,
            fontWeight: FontWeight.w700,
            fontSize: 15,
          ),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: ButtonStyle(
        padding: const WidgetStatePropertyAll(buttonPadding),
        shape: buttonShape,
        side: WidgetStatePropertyAll(BorderSide(color: b.line, width: 1.5)),
        foregroundColor: WidgetStatePropertyAll(b.ink),
        textStyle: const WidgetStatePropertyAll(
          TextStyle(
            fontFamily: textFont,
            fontWeight: FontWeight.w600,
            fontSize: 15,
          ),
        ),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: ButtonStyle(
        shape: buttonShape,
        textStyle: const WidgetStatePropertyAll(
          TextStyle(fontFamily: textFont, fontWeight: FontWeight.w600),
        ),
      ),
    ),
    chipTheme: ChipThemeData(
      backgroundColor: b.surface,
      side: BorderSide(color: b.line),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      labelStyle: TextStyle(color: b.ink, fontWeight: FontWeight.w600),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: b.surface,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      height: 68,
      indicatorColor: b.tint,
      iconTheme: WidgetStateProperty.resolveWith(
        (s) => IconThemeData(
          color: s.contains(WidgetState.selected) ? b.blue : b.muted,
        ),
      ),
      labelTextStyle: WidgetStateProperty.resolveWith(
        (s) => TextStyle(
          fontSize: 12,
          fontWeight: s.contains(WidgetState.selected)
              ? FontWeight.w700
              : FontWeight.w500,
          color: s.contains(WidgetState.selected) ? b.ink : b.muted,
        ),
      ),
    ),
    navigationRailTheme: NavigationRailThemeData(
      backgroundColor: b.surface,
      indicatorColor: b.tint,
      selectedIconTheme: IconThemeData(color: b.blue),
      unselectedIconTheme: IconThemeData(color: b.muted),
      selectedLabelTextStyle: TextStyle(
        color: b.ink,
        fontWeight: FontWeight.w700,
        fontSize: 13,
      ),
      unselectedLabelTextStyle: TextStyle(color: b.muted, fontSize: 13),
    ),
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: b.surface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
      titleTextStyle: textTheme.titleLarge,
    ),
    popupMenuTheme: PopupMenuThemeData(
      color: b.surface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(color: b.line),
      ),
    ),
    progressIndicatorTheme: ProgressIndicatorThemeData(
      color: b.blue,
      linearTrackColor: b.tint,
    ),
  );
}
