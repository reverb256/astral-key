// ── Mosiac Identity i18n Engine ────────────────────────────────────────────
// Lightweight, dependency-free translation system for vanilla JS.
// Loads /i18n/{locale}.json and applies data-i18n* attributes.
// ──────────────────────────────────────────────────────────────────────────

const MosiacI18n = (() => {
  let _translations = {};
  let _locale = 'en';
  let _ready = null;

  const SUPPORTED = ['en', 'fr'];
  const DEFAULT = 'en';

  // Detect preferred locale
  // Precedence: ?lang= > identity_lang cookie > browser language > 'en'
  function _detect() {
    // 1. URL query param
    const params = new URLSearchParams(window.location.search);
    const qLang = params.get('lang');
    if (qLang && SUPPORTED.includes(qLang)) return qLang;
    // 2. Cookie
    const match = document.cookie.match(/(?:^|;\s*)identity_lang=(\w+)/);
    if (match && SUPPORTED.includes(match[1])) return match[1];
    // 3. Browser language
    const browser = (navigator.language || 'en').split('-')[0].toLowerCase();
    return SUPPORTED.includes(browser) ? browser : DEFAULT;
  }

  // Load locale JSON
  async function load(locale) {
    try {
      const res = await fetch(`/i18n/${locale}.json`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      _translations = await res.json();
      _locale = locale;
      document.documentElement.lang = locale;
      // Persist via cookie (30 days)
      document.cookie = `identity_lang=${locale}; path=/; max-age=2592000; SameSite=Lax`;
    } catch (err) {
      console.warn('[MosiacI18n] Failed to load locale "' + locale + '":', err.message);
      if (locale !== DEFAULT) {
        console.info('[MosiacI18n] Falling back to "' + DEFAULT + '"');
        await load(DEFAULT);
      }
    }
  }

  // Translate a dot-notation key
  function t(key, params = {}) {
    const val = key.split('.').reduce(
      (obj, k) => (obj != null && Object.prototype.hasOwnProperty.call(obj, k) ? obj[k] : null),
      _translations
    );
    if (val === null || val === undefined) return key;
    let str = String(val);
    for (const [k, v] of Object.entries(params)) {
      str = str.replace(new RegExp('\\{\\{' + k + '\\}\\}', 'g'), String(v));
    }
    return str;
  }

  // Apply data-i18n* attributes to DOM elements
  function applyDOM(root) {
    root = root || document;
    // Text content
    root.querySelectorAll('[data-i18n]').forEach(el => {
      const val = t(el.dataset.i18n);
      if (val !== el.dataset.i18n) el.textContent = val;
    });
    // innerHTML (use sparingly)
    root.querySelectorAll('[data-i18n-html]').forEach(el => {
      const val = t(el.dataset.i18nHtml);
      if (val !== el.dataset.i18nHtml) el.innerHTML = val;
    });
    // Placeholder
    root.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      const val = t(el.dataset.i18nPlaceholder);
      if (val !== el.dataset.i18nPlaceholder) el.placeholder = val;
    });
    // Title
    root.querySelectorAll('[data-i18n-title]').forEach(el => {
      const val = t(el.dataset.i18nTitle);
      if (val !== el.dataset.i18nTitle) el.title = val;
    });
  }

  // Initialise
  function init() {
    if (_ready) return _ready;
    _ready = (async () => {
      const locale = _detect();
      await load(locale);
      if (document.readyState === 'loading') {
        await new Promise(r => document.addEventListener('DOMContentLoaded', r, { once: true }));
      }
      applyDOM();
    })();
    return _ready;
  }

  // Change locale at runtime
  async function setLocale(locale) {
    if (!SUPPORTED.includes(locale)) locale = DEFAULT;
    document.cookie = 'identity_lang=' + locale + '; path=/; max-age=2592000; SameSite=Lax';
    await load(locale);
    applyDOM();
    window.location.reload();
  }

  return {
    init,
    load,
    setLocale,
    t,
    applyDOM,
    get locale() { return _locale; },
    get supported() { return [...SUPPORTED]; },
  };
})();

window.MosiacI18n = MosiacI18n;
window.t = (key, params) => MosiacI18n.t(key, params);
MosiacI18n.init();
