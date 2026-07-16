/**
 * Mosaic Connections Module — Follow/unfollow buttons, follower/following
 * lists, and block management UI.
 *
 * Usage:
 *   const conn = new MosaicConnections('#connections-container', currentPubkey);
 *   await conn.loadConnections(targetPubkey);
 *
 * Provides follow/unfollow/block UI and displays followers/following lists.
 */

// ─── Base URL ──────────────────────────────────────────────────────────────

const MOSAIC_API_CONN = '';

class MosaicConnections {
  constructor(containerSelector, currentPubkey) {
    this.container = document.querySelector(containerSelector);
    this.currentPubkey = currentPubkey || null;
    this.viewingPubkey = null;
    this.followers = [];
    this.following = [];
    this.blocks = [];
    this.isFollowing = false;
    this.isBlocked = false;
    this.activeTab = 'following';
  }

  // ─── API Helpers ─────────────────────────────────────────────────────────

  async _fetch(path, options = {}) {
    const res = await fetch(`${MOSAIC_API_CONN}/mosaic${path}`, {
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
    const token = document.cookie.split('; ')
      .find(r => r.startsWith('token='))
      ?.split('=')[1];
    if (!token) return {};
    return { 'Authorization': `Bearer ${token}` };
  }

  // ─── Load Connections ────────────────────────────────────────────────────

  async loadConnections(pubkey) {
    this.viewingPubkey = pubkey;
    this.container.innerHTML = '<div class="connections-loading">Loading connections…</div>';

    try {
      // Load connections data
      const data = await this._fetch(`/connections/${pubkey}`);

      this.followers = data.followers || [];
      this.following = data.following || [];

      // Check follow/block status if current user is viewing someone else
      if (this.currentPubkey && this.currentPubkey !== pubkey) {
        this.isFollowing = this.following.includes(this.currentPubkey) ||
          data.isFollowing === true;
        this.isBlocked = data.isBlocked === true;
      }

      // Load blocks if viewing own profile
      if (this.currentPubkey === pubkey) {
        try {
          const blocksData = await this._fetch(`/connections/${pubkey}?include_blocks=true`);
          this.blocks = blocksData.blocks || [];
        } catch { /* blocks may not be public */ }
      }

      this.render();
    } catch (err) {
      this.container.innerHTML = `<div class="connections-error">Failed to load connections: ${err.message}</div>`;
    }
  }

  // ─── Actions ─────────────────────────────────────────────────────────────

  async follow(pubkey) {
    try {
      await this._fetch('/follow', {
        method: 'POST',
        headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
        body: JSON.stringify({ followee: pubkey }),
      });
      this.isFollowing = true;
      this.followers.unshift(this.currentPubkey);
      this.updateFollowButton();
    } catch (err) {
      alert('Failed to follow: ' + err.message);
    }
  }

  async unfollow(pubkey) {
    try {
      await this._fetch('/follow', {
        method: 'DELETE',
        headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
        body: JSON.stringify({ followee: pubkey }),
      });
      this.isFollowing = false;
      this.followers = this.followers.filter(f => f !== this.currentPubkey);
      this.updateFollowButton();
    } catch (err) {
      alert('Failed to unfollow: ' + err.message);
    }
  }

  async block(pubkey) {
    if (!confirm(`Block ${this._truncatePubkey(pubkey)}? This will also unfollow them.`)) return;

    try {
      await this._fetch('/block', {
        method: 'POST',
        headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
        body: JSON.stringify({ blockee: pubkey, reason: '' }),
      });
      this.isBlocked = true;
      this.isFollowing = false;
      this.render();
    } catch (err) {
      alert('Failed to block: ' + err.message);
    }
  }

  async unblock(pubkey) {
    try {
      await this._fetch('/block', {
        method: 'DELETE',
        headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
        body: JSON.stringify({ blockee: pubkey }),
      });
      this.isBlocked = false;
      this.blocks = this.blocks.filter(b => b.blockee !== pubkey);
      this.render();
    } catch (err) {
      alert('Failed to unblock: ' + err.message);
    }
  }

  // ─── Render ──────────────────────────────────────────────────────────────

  render() {
    const isOwn = this.currentPubkey === this.viewingPubkey;
    const isOther = this.currentPubkey && this.currentPubkey !== this.viewingPubkey;

    let html = '<div class="mosaic-connections">';

    // Action buttons (viewing someone else's profile)
    if (isOther) {
      html += `
        <div class="connections-actions">
          <button id="conn-follow-btn" class="${this.isFollowing ? 'btn-secondary' : 'btn-primary'}">
            ${this.isFollowing ? '✓ Following' : '+ Follow'}
          </button>
          <button id="conn-block-btn" class="btn-danger ${this.isBlocked ? 'blocked' : ''}">
            ${this.isBlocked ? '✓ Blocked' : 'Block'}
          </button>
        </div>
      `;
    }

    // Counts
    html += `
      <div class="connections-counts">
        <span><strong>${this.followers.length}</strong> followers</span>
        <span><strong>${this.following.length}</strong> following</span>
      </div>
    `;

    // Tabs
    html += `
      <div class="connections-tabs">
        <button class="conn-tab ${this.activeTab === 'following' ? 'active' : ''}" data-tab="following">Following</button>
        <button class="conn-tab ${this.activeTab === 'followers' ? 'active' : ''}" data-tab="followers">Followers</button>
        ${isOwn && this.blocks.length > 0 ? `<button class="conn-tab ${this.activeTab === 'blocks' ? 'active' : ''}" data-tab="blocks">Blocked (${this.blocks.length})</button>` : ''}
      </div>
    `;

    // Active tab content
    html += '<div class="connections-list">';
    if (this.activeTab === 'following') {
      if (this.following.length === 0) {
        html += '<p class="connections-empty">Not following anyone yet.</p>';
      } else {
        for (const pubkey of this.following) {
          html += this._renderConnectionItem(pubkey);
        }
      }
    } else if (this.activeTab === 'followers') {
      if (this.followers.length === 0) {
        html += '<p class="connections-empty">No followers yet.</p>';
      } else {
        for (const pubkey of this.followers) {
          html += this._renderConnectionItem(pubkey);
        }
      }
    } else if (this.activeTab === 'blocks' && isOwn) {
      if (this.blocks.length === 0) {
        html += '<p class="connections-empty">No blocked users.</p>';
      } else {
        for (const block of this.blocks) {
          html += this._renderBlockItem(block);
        }
      }
    }
    html += '</div>';

    html += '</div>';
    this.container.innerHTML = html;

    // Bind events
    this._bindActionButtons();
    this._bindTabs();
  }

  updateFollowButton() {
    const btn = this.container.querySelector('#conn-follow-btn');
    if (btn) {
      btn.textContent = this.isFollowing ? '✓ Following' : '+ Follow';
      btn.className = this.isFollowing ? 'btn-secondary' : 'btn-primary';
    }
  }

  _renderConnectionItem(pubkey) {
    const isOwn = this.currentPubkey === pubkey;
    return `
      <div class="connection-item" data-pubkey="${pubkey}">
        <span class="connection-pubkey">${this._truncatePubkey(pubkey)}</span>
        ${isOwn ? '<span class="connection-tag">you</span>' : ''}
        <button class="connection-view-btn btn-small" data-pubkey="${pubkey}">View Profile</button>
      </div>
    `;
  }

  _renderBlockItem(block) {
    return `
      <div class="connection-item blocked-item" data-pubkey="${block.blockee}">
        <span class="connection-pubkey">${this._truncatePubkey(block.blockee)}</span>
        ${block.reason ? `<span class="block-reason">${this._escapeHTML(block.reason)}</span>` : ''}
        <button class="connection-unblock-btn btn-small" data-pubkey="${block.blockee}">Unblock</button>
      </div>
    `;
  }

  // ─── Event Binding ──────────────────────────────────────────────────────

  _bindActionButtons() {
    const followBtn = this.container.querySelector('#conn-follow-btn');
    if (followBtn && this.viewingPubkey) {
      followBtn.addEventListener('click', () => {
        if (this.isFollowing) {
          this.unfollow(this.viewingPubkey);
        } else {
          this.follow(this.viewingPubkey);
        }
      });
    }

    const blockBtn = this.container.querySelector('#conn-block-btn');
    if (blockBtn && this.viewingPubkey) {
      blockBtn.addEventListener('click', () => {
        if (this.isBlocked) {
          this.unblock(this.viewingPubkey);
        } else {
          this.block(this.viewingPubkey);
        }
      });
    }

    // View profile buttons
    this.container.querySelectorAll('.connection-view-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const pubkey = btn.dataset.pubkey;
        if (this.onViewProfile) {
          this.onViewProfile(pubkey);
        }
      });
    });

    // Unblock buttons
    this.container.querySelectorAll('.connection-unblock-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const pubkey = btn.dataset.pubkey;
        this.unblock(pubkey);
      });
    });
  }

  _bindTabs() {
    this.container.querySelectorAll('.conn-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        this.activeTab = tab.dataset.tab;
        this.render();
      });
    });
  }

  // ─── Callbacks ───────────────────────────────────────────────────────────

  onViewProfile(pubkey) {
    // Override this to navigate to a profile view
    console.log('View profile:', pubkey);
  }

  // ─── Helpers ─────────────────────────────────────────────────────────────

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

// ─── Module export ─────────────────────────────────────────────────────────

(function() {
  if (typeof window !== 'undefined') {
    window.MosaicConnections = MosaicConnections;
  }
})();
