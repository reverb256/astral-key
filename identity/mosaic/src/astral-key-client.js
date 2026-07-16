'use strict';

/**
 * Astral Key HTTP Client — proxies FIDO2/Web3 auth requests to
 * astral-key (reverb256/astral-key), a Rust/Axum auth microservice.
 *
 * Each method returns the *same shape* as the corresponding passkey.js
 * function so that routes-mosaic.js can be a drop-in replacement.
 *
 * Uses Node 18+ global `fetch()` — zero additional dependencies.
 */

class AstralKeyClient {
  /**
   * @param {string} baseUrl — e.g. "http://localhost:3001"
   */
  constructor(baseUrl) {
    this.baseUrl = baseUrl.replace(/\/+$/, '');
  }

  /* ── Internal helpers ─────────────────────────────────── */

  async _fetch(path, options = {}) {
    const url = `${this.baseUrl}${path}`;
    const res = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
        ...(options.headers || {}),
      },
    });

    let body;
    const contentType = res.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      body = await res.json();
    } else {
      const text = await res.text();
      body = { error: text };
    }

    if (!res.ok) {
      const err = new Error(body.error || body.message || `astral-key HTTP ${res.status}`);
      err.status = res.status;
      err.body = body;
      throw err;
    }

    return body;
  }

  /* ── Health ───────────────────────────────────────────── */

  async health() {
    return this._fetch('/health');
  }

  /* ── Identity ─────────────────────────────────────────── */

  async listIdentities() {
    return this._fetch('/identity');
  }

  async currentIdentity() {
    return this._fetch('/identity/current');
  }

  async generateQR(pubkey) {
    return this._fetch(`/identity/qr/${encodeURIComponent(pubkey)}`);
  }

  async verifySignature(envelope) {
    return this._fetch('/identity/verify', {
      method: 'POST',
      body: JSON.stringify({ envelope }),
    });
  }

  /* ── Contacts ─────────────────────────────────────────── */

  async listContacts() {
    return this._fetch('/contacts');
  }

  async upsertContact(pubkey, label, discoveredVia) {
    return this._fetch('/contacts', {
      method: 'POST',
      body: JSON.stringify({ pubkey, label, discovered_via: discoveredVia }),
    });
  }

  async scanContact(data) {
    return this._fetch('/contacts/scan', {
      method: 'POST',
      body: JSON.stringify({ data }),
    });
  }

  async deleteContact(pubkey) {
    return this._fetch(`/contacts/${encodeURIComponent(pubkey)}`, {
      method: 'DELETE',
    });
  }

  /* ── FIDO2 Registration ──────────────────────────────── */

  /**
   * Start WebAuthn credential registration.
   * Returns the same shape as passkey.beginRegistration()
   * so the route handler is a drop-in swap.
   */
  async registerBegin({ label } = {}) {
    const body = await this._fetch('/auth/fido2/register/options', {
      method: 'POST',
      body: JSON.stringify({ label: label || null }),
    });
    return {
      identityId: body.identityId,
      pubkey: body.pubkey,
      options: body.options,
    };
  }

  /**
   * Complete WebAuthn credential registration.
   * Returns the same shape as passkey.completeRegistration().
   */
  async registerComplete({ challenge, credential, nickname }) {
    const body = await this._fetch('/auth/fido2/register/verify', {
      method: 'POST',
      body: JSON.stringify({
        challenge,
        credential,
        nickname: nickname || null,
      }),
    });
    return {
      verified: body.verified,
      identityId: body.identityId,
      pubkey: body.pubkey,
    };
  }

  /* ── FIDO2 Authentication ──────────────────────────────── */

  /**
   * Start WebAuthn assertion / login.
   * Returns the same shape as passkey.beginAuthentication().
   */
  async loginBegin() {
    const body = await this._fetch('/auth/fido2/login/start', {
      method: 'POST',
    });
    return {
      options: body.options,
      challenge: body.options?.challenge || body.challenge,
    };
  }

  /**
   * Complete WebAuthn assertion / login.
   * Returns the same shape as passkey.completeAuthentication().
   */
  async loginComplete({ credential }) {
    const body = await this._fetch('/auth/fido2/login/complete', {
      method: 'POST',
      body: JSON.stringify({ credential }),
    });
    return {
      verified: body.verified,
      identityId: body.identityId,
      pubkey: body.pubkey,
      sessionToken: body.sessionToken,
    };
  }

  /* ── FIDO2 Credential Management ───────────────────────── */

  /**
   * List all registered FIDO2 credentials.
   */
  async getCredentials() {
    return this._fetch('/auth/fido2/credentials');
  }

  /**
   * Delete a FIDO2 credential by its ID.
   */
  async deleteCredential(id) {
    return this._fetch(`/auth/fido2/credentials/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    });
  }

  /* ── Web3 / SIWE ──────────────────────────────────────── */

  /**
   * Get a SIWE (Sign-In with Ethereum) nonce.
   */
  async web3SiweNonce() {
    return this._fetch('/auth/web3/siwe/nonce', { method: 'POST' });
  }

  /**
   * Verify a SIWE signature.
   */
  async web3SiweVerify({ message, signature }) {
    return this._fetch('/auth/web3/siwe/verify', {
      method: 'POST',
      body: JSON.stringify({ message, signature }),
    });
  }

  /**
   * List linked Web3 wallets.
   */
  async getWeb3Wallets() {
    return this._fetch('/auth/web3/wallets');
  }
}

module.exports = { AstralKeyClient };
