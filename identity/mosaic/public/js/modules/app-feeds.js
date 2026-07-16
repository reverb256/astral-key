/**
 * Mosaic Feeds Module — Timeline view, feed composer, reaction buttons,
 * pagination via cursor.
 *
 * Usage:
 *   const feeds = new MosaicFeeds('#feeds-container', currentPubkey);
 *   await feeds.loadFeed('recent', { limit: 30 });
 *
 * The module supports multiple feed algorithms and infinite-scroll pagination.
 */

// ─── Base URL ──────────────────────────────────────────────────────────────

const MOSAIC_API_FEED = '';

class MosaicFeeds {
  constructor(containerSelector, currentPubkey) {
    this.container = document.querySelector(containerSelector);
    this.currentPubkey = currentPubkey || null;
    this.currentAlgo = 'recent';
    this.cursor = null;
    this.hasMore = true;
    this.loading = false;
    this.posts = [];
    this._boundScroll = null;
  }

  // ─── API Helpers ─────────────────────────────────────────────────────────

  async _fetch(path, options = {}) {
    const res = await fetch(`${MOSAIC_API_FEED}/mosaic${path}`, {
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

  // ─── Feed Loading ────────────────────────────────────────────────────────

  async loadFeed(algo, params = {}) {
    this.currentAlgo = algo || 'recent';
    this.cursor = null;
    this.hasMore = true;
    this.posts = [];
    this.container.innerHTML = '<div class="feeds-loading">Loading feed…</div>';
    await this._loadMore(params);
  }

  async loadMore() {
    if (this.loading || !this.hasMore) return;
    await this._loadMore();
  }

  async _loadMore(extraParams = {}) {
    this.loading = true;

    try {
      const query = new URLSearchParams({
        algo: this.currentAlgo,
        limit: String(extraParams.limit || 30),
      });
      if (this.cursor) query.set('cursor', this.cursor);

      const data = await this._fetch(`/feed?${query}`);
      const posts = data.posts || data;

      if (posts.length === 0) {
        this.hasMore = false;
      } else {
        this.posts.push(...posts);
        // Update cursor from last post
        const lastPost = posts[posts.length - 1];
        this.cursor = lastPost.created_at;
      }

      this.render();
    } catch (err) {
      this.container.innerHTML = `<div class="feeds-error">Failed to load feed: ${err.message}</div>`;
    } finally {
      this.loading = false;
    }
  }

  // ─── Render ──────────────────────────────────────────────────────────────

  render() {
    if (this.posts.length === 0) {
      this.container.innerHTML = `
        <div class="feeds-empty">
          <p>No posts yet.</p>
          ${this.currentPubkey ? '<button class="btn-primary" id="feeds-compose-btn">Create the first post</button>' : ''}
        </div>
      `;
      this._bindComposeButton();
      return;
    }

    // Build feed HTML
    let html = '';

    // Composer at top if logged in
    if (this.currentPubkey) {
      html += `
        <div class="feeds-composer">
          <textarea id="feeds-composer-input" placeholder="What's happening?" maxlength="2000" rows="3"></textarea>
          <div class="feeds-composer-actions">
            <button id="feeds-composer-submit" class="btn-primary">Post</button>
            <span class="feeds-composer-char-count" id="feeds-char-count">0</span>
          </div>
        </div>
      `;
    }

    // Algo selector
    html += `
      <div class="feeds-algo-selector">
        <button class="feed-algo-btn ${this.currentAlgo === 'recent' ? 'active' : ''}" data-algo="recent">Recent</button>
        <button class="feed-algo-btn ${this.currentAlgo === 'local' ? 'active' : ''}" data-algo="local">Local</button>
        <button class="feed-algo-btn ${this.currentAlgo === 'friends' ? 'active' : ''}" data-algo="friends">Friends</button>
      </div>
    `;

    // Post list
    html += '<div class="feeds-posts">';
    for (const post of this.posts) {
      html += this._renderPost(post);
    }
    html += '</div>';

    // Load more trigger
    if (this.hasMore) {
      html += '<div class="feeds-load-more"><button id="feeds-load-more-btn" class="btn-secondary">Load more</button></div>';
    }

    this.container.innerHTML = html;

    // Bind events
    this._bindComposerEvents();
    this._bindAlgoButtons();
    this._bindLoadMore();
    this._bindReactionButtons();
    this._bindInfiniteScroll();
  }

  _renderPost(post) {
    const reactions = post.reactions || { likes: 0, reposts: 0 };
    const hasLiked = post.userReaction?.like || false;
    const hasReposted = post.userReaction?.repost || false;
    const pubkey = post.pubkey || '';
    const displayName = post.display_name || this._truncatePubkey(pubkey);
    const createdAt = post.created_at ? new Date(post.created_at + 'Z').toLocaleString() : '';

    return `
      <div class="feed-post" data-cid="${post.cid}">
        <div class="feed-post-header">
          <span class="feed-post-author">${this._escapeHTML(displayName)}</span>
          <span class="feed-post-pubkey">${this._truncatePubkey(pubkey)}</span>
          <span class="feed-post-time">${createdAt}</span>
        </div>
        <div class="feed-post-content">${this._escapeHTML(post.content || '')}</div>
        ${post.reply_to ? `<div class="feed-post-reply">↻ Reply to ${this._truncatePubkey(post.reply_to)}</div>` : ''}
        <div class="feed-post-actions">
          <button class="feed-reaction-btn ${hasLiked ? 'active' : ''}" data-action="like" data-cid="${post.cid}">
            ♥ <span class="reaction-count">${reactions.likes}</span>
          </button>
          <button class="feed-reaction-btn ${hasReposted ? 'active' : ''}" data-action="repost" data-cid="${post.cid}">
            ↻ <span class="reaction-count">${reactions.reposts}</span>
          </button>
        </div>
      </div>
    `;
  }

  // ─── Event Binding ──────────────────────────────────────────────────────

  _bindComposeButton() {
    const btn = this.container.querySelector('#feeds-compose-btn');
    if (btn) btn.addEventListener('click', () => this.loadFeed(this.currentAlgo));
  }

  _bindComposerEvents() {
    const input = this.container.querySelector('#feeds-composer-input');
    const submit = this.container.querySelector('#feeds-composer-submit');
    const counter = this.container.querySelector('#feeds-char-count');

    if (input && counter) {
      input.addEventListener('input', () => {
        counter.textContent = input.value.length;
      });
    }

    if (submit) {
      submit.addEventListener('click', async () => {
        const content = input?.value.trim();
        if (!content) return;
        try {
          await this._fetch('/feed/post', {
            method: 'POST',
            headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
            body: JSON.stringify({ content }),
          });
          // Reload feed to show new post
          await this.loadFeed(this.currentAlgo);
        } catch (err) {
          alert('Failed to post: ' + err.message);
        }
      });
    }
  }

  _bindAlgoButtons() {
    this.container.querySelectorAll('.feed-algo-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const algo = btn.dataset.algo;
        await this.loadFeed(algo);
      });
    });
  }

  _bindLoadMore() {
    const btn = this.container.querySelector('#feeds-load-more-btn');
    if (btn) {
      btn.addEventListener('click', () => this.loadMore());
    }
  }

  _bindReactionButtons() {
    this.container.querySelectorAll('.feed-reaction-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (!this.currentPubkey) {
          alert('You need an active identity to react');
          return;
        }

        const cid = btn.dataset.cid;
        const action = btn.dataset.action;
        const isActive = btn.classList.contains('active');

        try {
          if (isActive) {
            await this._fetch('/feed/react', {
              method: 'DELETE',
              headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
              body: JSON.stringify({ cid, type: action }),
            });
          } else {
            await this._fetch('/feed/react', {
              method: 'POST',
              headers: { ...this._getAuthHeaders(), 'Content-Type': 'application/json' },
              body: JSON.stringify({ cid, type: action }),
            });
          }

          // Update UI immediately
          const countEl = btn.querySelector('.reaction-count');
          let count = parseInt(countEl.textContent || '0');
          if (isActive) {
            count = Math.max(0, count - 1);
            btn.classList.remove('active');
          } else {
            count += 1;
            btn.classList.add('active');
          }
          countEl.textContent = count;
        } catch (err) {
          alert('Failed to update reaction: ' + err.message);
        }
      });
    });
  }

  _bindInfiniteScroll() {
    // Remove old observer
    if (this._boundScroll) {
      window.removeEventListener('scroll', this._boundScroll);
    }

    this._boundScroll = () => {
      if (this.loading || !this.hasMore) return;
      const rect = this.container.getBoundingClientRect();
      if (rect.bottom < window.innerHeight + 200) {
        this.loadMore();
      }
    };

    window.addEventListener('scroll', this._boundScroll, { passive: true });
  }

  // ─── Cleanup ─────────────────────────────────────────────────────────────

  destroy() {
    if (this._boundScroll) {
      window.removeEventListener('scroll', this._boundScroll);
    }
    this.container.innerHTML = '';
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
    window.MosaicFeeds = MosaicFeeds;
  }
})();
