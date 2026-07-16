'use strict';

/**
 * Mosiac Identity i18n — lightweight bilingual helper.
 * Loads EN/FR string maps and resolves by Accept-Language header.
 */
const fs = require('fs');
const path = require('path');

// Cache loaded locales
const _cache = {};

const LOCALE_DIR = path.join(__dirname);

function loadLocale(locale) {
  if (_cache[locale]) return _cache[locale];
  try {
    const filePath = path.join(LOCALE_DIR, `${locale}.json`);
    if (!fs.existsSync(filePath)) return null;
    const raw = fs.readFileSync(filePath, 'utf-8');
    _cache[locale] = JSON.parse(raw);
    return _cache[locale];
  } catch {
    return null;
  }
}

/**
 * Resolve locale from Express request.
 * Precedence: ?lang= > identity_lang cookie > Accept-Language > 'en'
 */
function detectLocale(req) {
  // 1. Query param
  const qLang = req?.query?.lang;
  if (qLang && ['en', 'fr'].includes(qLang)) return qLang;
  // 2. Cookie
  const cookie = req?.headers?.cookie;
  if (cookie) {
    const match = cookie.match(/(?:^|;\s*)identity_lang=(\w+)/);
    if (match && ['en', 'fr'].includes(match[1])) return match[1];
  }
  // 3. Accept-Language header
  const acceptLang = req?.headers?.['accept-language'];
  if (acceptLang) {
    const preferred = acceptLang.split(',')[0]?.split('-')[0]?.toLowerCase();
    if (preferred && ['en', 'fr'].includes(preferred)) return preferred;
  }
  return 'en';
}

/**
 * Translate a dot-notation key for a given locale.
 * Falls back to the key name, then 'en'.
 */
function t(key, locale = 'en') {
  const translations = loadLocale(locale) || loadLocale('en');
  if (!translations) return key;
  const val = key.split('.').reduce(
    (obj, k) => (obj != null && typeof obj === 'object' && Object.prototype.hasOwnProperty.call(obj, k) ? obj[k] : null),
    translations
  );
  return val !== null && val !== undefined ? String(val) : key;
}

/**
 * Build a bilingual error object: { error: { en, fr }, code }
 */
function bilingualError(key, code) {
  return {
    error: {
      en: t(key, 'en'),
      fr: t(key, 'fr'),
    },
    code: code || 'AUTH_000',
  };
}

module.exports = { detectLocale, t, loadLocale, bilingualError };
