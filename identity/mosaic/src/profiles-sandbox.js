'use strict';

/**
 * Mosaic Profile Sandbox — CSP headers and HTML template for rendering
 * user-supplied HTML/CSS in a sandboxed iframe.
 *
 * CSP rules:
 *   - default-src 'none'
 *   - no script execution (script-src 'none')
 *   - no XMLHttpRequest/fetch (connect-src 'none')
 *   - no foreignObject (no SVG <foreignObject> that could run scripts)
 *   - no frame-src (can't embed another iframe)
 *   - style-src 'unsafe-inline' (user CSS needs inline styles)
 *   - img-src 'self' data: (allow avatars and data URIs)
 *   - font-src 'self' data: (allow custom fonts if any)
 */

/**
 * Returns Content-Security-Policy header value for profile iframes.
 * @returns {string}
 */
function sandboxHeaders() {
  return [
    "default-src 'none'",
    "script-src 'none'",
    "connect-src 'none'",
    "style-src 'unsafe-inline'",
    "img-src 'self' data:",
    "font-src 'self' data:",
    "frame-src 'none'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join('; ');
}

/**
 * Create a sandboxed HTML document from a profile manifest.
 * Wraps the user's HTML and CSS in a minimal document with CSP via meta tag.
 *
 * @param {object} profileManifest - Parsed profile manifest
 * @returns {string} Full HTML document string safe for iframe srcdoc
 */
function createSandboxedHTML(profileManifest) {
  const content = profileManifest.content || {};
  const userHtml = content.html || '';
  const userCss = content.css || '';
  const displayName = profileManifest.display_name || 'Anonymous';
  const bio = profileManifest.bio || '';
  const links = profileManifest.links || [];

  // Build links HTML
  const linksHtml = links.length > 0
    ? `<div class="profile-links">${
        links.map(l => `<a href="${escapeHTML(l.url)}" rel="nofollow noopener" target="_blank">${escapeHTML(l.label)}</a>`).join(' · ')
      }</div>`
    : '';

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${sandboxHeaders()}">
<style>
  /* CSS reset for profile sandbox */
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
  html, body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: transparent;
    color: #e0e0e0;
    line-height: 1.5;
    min-height: 100%;
  }
  a { color: #58a6ff; text-decoration: none; }
  a:hover { text-decoration: underline; }
  img { max-width: 100%; height: auto; }

  /* Default profile layout */
  .profile-header { padding: 16px; }
  .profile-name { font-size: 1.5rem; font-weight: 700; margin-bottom: 4px; }
  .profile-bio { font-size: 0.9rem; opacity: 0.8; margin-bottom: 8px; }
  .profile-links { font-size: 0.85rem; margin-bottom: 12px; }
  .profile-content { padding: 0 16px 16px; }

  /* User-supplied CSS injected below */
  ${userCss}
</style>
</head>
<body>
<div class="profile-header">
  <div class="profile-name">${escapeHTML(displayName)}</div>
  ${bio ? `<div class="profile-bio">${escapeHTML(bio)}</div>` : ''}
  ${linksHtml}
</div>
<div class="profile-content">
  ${userHtml}
</div>
</body>
</html>`;
}

/**
 * Escape HTML entities to prevent injection.
 */
function escapeHTML(str) {
  if (typeof str !== 'string') return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#x27;');
}

module.exports = {
  sandboxHeaders,
  createSandboxedHTML,
};
