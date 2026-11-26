// Google Analytics 4 лише для сайту uautonomy.app. Застосунок аналітики не має.
// Не вантажимо нічого, якщо браузер просить не стежити (GPC чи Do Not Track)
// або людина вимкнула аналітику на сторінці /privacy.
(function () {
  var ID = 'G-8RQB5YYHY7';
  var off = false;
  try { off = localStorage.getItem('ua-analytics-off') === '1'; } catch (e) {}
  if (off || navigator.globalPrivacyControl === true || navigator.doNotTrack === '1' || window.doNotTrack === '1') {
    window['ga-disable-' + ID] = true;
    return;
  }
  window.dataLayer = window.dataLayer || [];
  function gtag() { dataLayer.push(arguments); }
  window.gtag = gtag;
  // Рекламне зберігання й персоналізація вимкнені назавжди: сайт не показує
  // реклами і не передає даних для неї.
  gtag('consent', 'default', {
    ad_storage: 'denied',
    ad_user_data: 'denied',
    ad_personalization: 'denied',
    analytics_storage: 'granted'
  });
  gtag('js', new Date());
  gtag('config', ID, {
    allow_google_signals: false,
    allow_ad_personalization_signals: false
  });
  // Завантаження йдуть через короткі адреси (/android, /windows ...), тож
  // автоматичне відстеження файлів їх не бачить. Рахуємо самі.
  var DL = { '/android': 'android', '/apk': 'android', '/android-32': 'android-32', '/android-x86': 'android-x86',
             '/windows': 'windows', '/windows-zip': 'windows-zip' };
  document.addEventListener('click', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (!a || a.host !== location.host || !DL[a.pathname]) return;
    gtag('event', 'app_download', { platform: DL[a.pathname], link_id: a.id || undefined, transport_type: 'beacon' });
  });
  var s = document.createElement('script');
  s.async = true;
  s.src = 'https://www.googletagmanager.com/gtag/js?id=' + ID;
  document.head.appendChild(s);
})();
