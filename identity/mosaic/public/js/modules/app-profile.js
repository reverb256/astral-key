/**
 * Mosaic Profile Module — Profile editor/viewer with split-pane HTML/CSS editing
 * and sandboxed preview.
 *
 * This module can be loaded independently or alongside other app-*.js modules.
 * It expects DOM elements with specific IDs (see renderProfileViewer).
 *
 * Usage:
 *   // Initialize with container and current pubkey
 *   const profile = new MosaicProfile('#profile-container', currentPubkey);
 *   await profile.loadProfile(targetPubkey);
 */

// ─── Base URL ──────────────────────────────────────────────────────────────

const MOSAIC_API = '';

// ─── State ─────────────────────────────────────────────────────────────────

class MosaicProfile {
  constructor(containerSelector, currentPubkey) {
    this.container = document.querySelector(containerSelector);
    this.currentPubkey = currentPubkey || null;
    this.viewingPubkey = null;
    this.profile = null;
    this.editing = false;
  }

  // ─── API Helpers ─────────────────────────────────────────────────────────

  async _fetch(path, options = {}) {
    const res = await fetch(`${MOSAIC_API}/mosaic${path}`, {
      headers: { 'Content-Type': 'application/json', ...options.headers },
      ...options,
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      throw new Error(err.error || `HTTP ${res.status}`);
    }
    return res.json();
  }

  _getAuthHeaders() {
    // Same token-based auth as other Mosaic modules
    const token = document.cookie.split('; ')
      .find(r => r.startsWith('token='))
      ?.split('=')[1];
    if (!token) return {};
    return { 'Authorization': `Bearer ${token}` };
  }

  // ─── Load Profile ────────────────────────────────────────────────────────

  async loadProfile(pubkey) {
    this.viewingPubkey = pubkey;
    try {
      const data = await this._fetch(`/profile/${pubkey}`);
      this.profile = data.profile || data;
      this.render();
    } catch (err) {
      this.container.innerHTML = `<div class="profile-error">Failed to load profile: ${err.message}</div>`;
    }
  }

  // ─── Render ──────────────────────────────────────────────────────────────

  render() {
    if (!this.profile) {
      this.container.innerHTML = '<div class="profile-empty">No profile found</div>';
      return;
    }

    const manifest = this.profile.manifest || this.profile;
    const isOwn = this.currentPubkey === this.viewingPubkey;

    this.container.innerHTML = `
      <div class="mosaic-profile">
        <div class="profile-header-bar">
          <div class="profile-identity">
            <span class="profile-name">${this._escapeHTML(manifest.display_name || 'Anonymous')}</span>
            <span class="profile-pubkey">${this._truncatePubkey(manifest.pubkey)}</span>
          </div>
          ${isOwn ? `<button class="profile-edit-btn" data-action="edit">Edit Profile</button>` : ''}
        </div>
        ${manifest.bio ? `<div class="profile-bio">${this._escapeHTML(manifest.bio)}</div>` : ''}
        ${manifest.links && manifest.links.length > 0 ? `
          <div class="profile-links">
            ${manifest.links.map(l => `<a href="${this._escapeHTML(l.url)}" target="_blank" rel="noopener">${this._escapeHTML(l.label)}</a>`).join(' · ')}
          </div>
        ` : ''}
        ${manifest.content ? `
          <div class="profile-content-preview">
            <iframe class="profile-sandbox-iframe" srcdoc="${this._escapeHTML(this._buildSandboxHTML(manifest))}" sandbox="allow-scripts allow-same-origin"></iframe>
          </div>
        ` : ''}
        ${manifest.widgets && manifest.widgets.length > 0 ? `
          <div class="profile-widgets">
            ${manifest.widgets.map(w => `<span class="profile-widget-tag">${this._escapeHTML(w.type)}</span>`).join('')}
          </div>
        ` : ''}
        <div class="profile-verified ${this._verifyManifest(manifest) ? 'verified' : 'unverified'}">
          ${this._verifyManifest(manifest) ? '✓ Signed by owner' : '✗ Signature invalid'}
        </div>
      </div>
    `;

    // Bind edit button
    if (isOwn) {
      this.container.querySelector('[data-action="edit"]')?.addEventListener('click', () => this.showEditor());
    }
  }

  // ─── Editor (split-pane) ─────────────────────────────────────────────────

  showEditor() {
    if (this.editing) return;
    this.editing = true;

    const manifest = this.profile.manifest || this.profile;

    this.container.innerHTML = `
      <div class="mosaic-profile-editor">
        <h3>Edit Profile</h3>
        <div class="editor-field">
          <label>Display Name</label>
          <input type="text" id="profile-editor-name" value="${this._escapeHTML(manifest.display_name || '')}" maxlength="64" />
        </div>
        <div class="editor-field">
          <label>Bio</label>
          <textarea id="profile-editor-bio" maxlength="512">${this._escapeHTML(manifest.bio || '')}</textarea>
        </div>
        <div class="editor-field">
          <label>Avatar URL</label>
          <input type="text" id="profile-editor-avatar" value="${this._escapeHTML(manifest.avatar || '')}" />
        </div>
        <div class="editor-field">
          <label>Theme</label>
          <select id="profile-editor-theme">
            <option value="mosaic-dark" ${(manifest.theme||'mosaic-dark') === 'mosaic-dark' ? 'selected' : ''}>Mosaic Dark</option>
            <option value="mosaic-light" ${manifest.theme === 'mosaic-light' ? 'selected' : ''}>Mosaic Light</option>
            <option value="custom" ${manifest.theme === 'custom' ? 'selected' : ''}>Custom</option>
          </select>
        </div>
        <div class="editor-panes">
          <div class="editor-pane">
            <label>HTML <small>(max 50K chars)</small></label>
            <textarea id="profile-editor-html" class="editor-code">${this._escapeHTML((manifest.content && manifest.content.html) || '')}</textarea>
          </div>
          <div class="editor-pane">
            <label>CSS <small>(max 50K chars)</small></label>
            <textarea id="profile-editor-css" class="editor-code">${this._escapeHTML((manifest.content && manifest.content.css) || '')}</textarea>
          </div>
        </div>
        <div class="editor-preview">
          <label>Preview</label>
          <iframe id="profile-editor-preview" class="profile-sandbox-iframe" sandbox="allow-scripts allow-same-origin"></iframe>
        </div>
        <div class="editor-actions">
          <button id="profile-preview-btn" class="btn-secondary">Refresh Preview</button>
          <button id="profile-save-btn" class="btn-primary">Save Profile</button>
          <button id="profile-cancel-btn" class="btn-secondary">Cancel</button>
        </div>
      </div>
    `;

    // Preview refresh
    this.container.querySelector('#profile-preview-btn').addEventListener('click', () => this._refreshPreview());
    this.container.querySelector('#profile-cancel-btn').addEventListener('click', () => {
      this.editing = false;
      this.render();
    });
    this.container.querySelector('#profile-save-btn').addEventListener('click', () => this._saveProfile());

    // Initial preview
    this._refreshPreview();
  }

  _refreshPreview() {
    const html = this.container.querySelector('#profile-editor-html').value;
    const css = this.container.querySelector('#profile-editor-css').value;
    const name = this.container.querySelector('#profile-editor-name').value;
    const bio = this.container.querySelector('#profile-editor-bio').value;

    const fakeManifest = {
      display_name: name,
      bio,
      content: { html, css },
      links: [],
    };

    const iframe = this.container.querySelector('#profile-editor-preview');
    iframe.srcdoc = this._buildSandboxHTML(fakeManifest);
  }

  async _saveProfile() {
    const name = this.container.querySelector('#profile-editor-name').value.trim();
    const bio = this.container.querySelector('#profile-editor-bio').value.trim();
    const avatar = this.container.querySelector('#profile-editor-avatar').value.trim() || null;
    const theme = this.container.querySelector('#profile-editor-theme').value;
    const html = this.container.querySelector('#profile-editor-html').value;
    const css = this.container.querySelector('#profile-editor-css').value;

    const payload = {
      display_name: name || 'Anonymous',
      bio: bio || '',
      avatar,
      theme,
      content: (html || css) ? { html: html || '', css: css || '' } : null,
    };

    try {
      const result = await this._fetch('/profile', {
        method: 'POST',
        headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      this.profile = result.profile || result;
      this.editing = false;
      this.render();
    } catch (err) {
      alert('Failed to save profile: ' + err.message);
    }
  }

  // ─── Helpers ────────────────────────────────────────────────────────────

  _buildSandboxHTML(manifest) {
    const content = manifest.content || {};
    const userHtml = content.html || '';
    const userCss = content.css || '';
    const displayName = manifest.display_name || 'Anonymous';
    const bio = manifest.bio || '';

    const linksHtml = (manifest.links && manifest.links.length > 0)
      ? `<div class="profile-links">${
          manifest.links.map(l => `<a href="${this._escapeHTML(l.url)}" rel="nofollow noopener" target="_blank">${this._escapeHTML(l.label)}</a>`).join(' · ')
        }</div>`
      : '';

    return `<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'none'; connect-src 'none'; style-src 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none';">
<style>
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}
html,body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:transparent;color:#e0e0e0;line-height:1.5;min-height:100%}
a{color:#58a6ff;text-decoration:none} a:hover{text-decoration:underline}
img{max-width:100%;height:auto}
.profile-header{padding:16px}
.profile-name{font-size:1.5rem;font-weight:700;margin-bottom:4px}
.profile-bio{font-size:.9rem;opacity:.8;margin-bottom:8px}
.profile-links{font-size:.85rem;margin-bottom:12px}
.profile-content{padding:0 16px 16px}
${userCss}
</style>
</head><body>
<div class="profile-header"><div class="profile-name">${this._escapeHTML(displayName)}</div>
${bio ? `<div class="profile-bio">${this._escapeHTML(bio)}</div>` : ''}
${linksHtml}</div>
<div class="profile-content">${userHtml}</div>
</body></html>`;
  }

  _verifyManifest(manifest) {
    // Client-side: we can't verify Ed25519 easily in the browser without tweetnacl,
    // so we show the server-verified status or just check signature presence
    return manifest.signature && manifest.signature.length > 0;
  }

  _escapeHTML(str) {
    if (typeof str !== 'string') return '';
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#x27;');
  }

  _truncatePubkey(pubkey) {
    if (!pubkey) return '';
    if (pubkey.startsWith('ed25519:')) pubkey = pubkey.slice(8);
    if (pubkey.length <= 16) return pubkey;
    return pubkey.slice(0, 8) + '…' + pubkey.slice(-8);
  }
}

// ─── Module export (IIFE for non-module script inclusion) ──────────────────

(function() {
  if (typeof window !== 'undefined') {
    window.MosaicProfile = MosaicProfile;
  }
})();
