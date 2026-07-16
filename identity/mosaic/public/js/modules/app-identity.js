/**
 * Mosaic Identity Layer — Frontend Module
 *
 * Handles:
 *   - WebAuthn registration ceremony (passkey creation)
 *   - WebAuthn authentication ceremony (passkey login)
 *   - Session management (token in cookie)
 *   - QR code display for pubkey sharing
 *   - Contact management via QR scan
 *   - Signing/verification demo
 */

// ─── Base URL ──────────────────────────────────────────────────────────────

const API = '';

// ─── DOM refs ──────────────────────────────────────────────────────────────

const $ = (id) => document.getElementById(id);

const views = {
  splash:              $('splash'),
  register:            $('register'),
  login:               $('login'),
  dashboard:           $('dashboard'),
  existingIdentities:  $('existing-identities'),
};

function showView(name) {
  Object.values(views).forEach(v => v?.classList.remove('active'));
  const view = views[name];
  if (view) view.classList.add('active');
}

// ─── Base64 / ArrayBuffer helpers ─────────────────────────────────────────

function base64URLToBytes(str) {
  const base64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const bin = atob(base64);
  const buf = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
  return buf;
}

function bytesToBase64URL(bytes) {
  const bin = String.fromCharCode(...bytes);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function coerceToBuffer(value) {
  if (typeof value === 'string') return base64URLToBytes(value).buffer;
  if (value instanceof ArrayBuffer) return value;
  if (value instanceof Uint8Array) return value.buffer;
  if (value?.buffer) return value.buffer;
  return value;
}

// ─── Connection status ────────────────────────────────────────────────────

async function checkConnection() {
  try {
    const res = await fetch(`/mosaic/health`);
    if (res.ok) {
      $('conn-status').textContent = 'connected';
      $('conn-status').classList.add('connected');
      return true;
    }
  } catch { /* offline */ }
  $('conn-status').textContent = 'disconnected';
  $('conn-status').classList.remove('connected');
  return false;
}

// ─── Registration Flow ─────────────────────────────────────────────────────

let registrationState = null;

async function beginRegistration() {
  showView('register');
  $('register-step1').classList.remove('hidden');
  $('register-step2').classList.add('hidden');
  $('register-complete').classList.add('hidden');

  try {
    const res = await fetch(`/api/auth/passkey/register/begin`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ label: null }),
    });

    if (!res.ok) throw new Error((await res.json()).error);
    const data = await res.json();

    // Display the generated identity info
    $('reg-pubkey').textContent = data.pubkey;
    $('reg-fingerprint').textContent = data.pubkeyHex.slice(0, 16) + '…';
    $('reg-uri').textContent = `mosaic://${data.pubkey}`;

    // Store state for passkey registration
    registrationState = data;

    $('register-step1').classList.add('hidden');
    $('register-step2').classList.remove('hidden');
  } catch (err) {
    alert('Registration failed: ' + err.message);
    showView('splash');
  }
}

async function registerPasskey() {
  if (!registrationState) return;

  try {
    const options = registrationState.options;

    // Convert to proper types for WebAuthn API
    options.challenge = base64URLToBytes(options.challenge);
    options.user.id = base64URLToBytes(options.user.id);
    if (options.excludeCredentials) {
      options.excludeCredentials = options.excludeCredentials.map(c => ({
        ...c,
        id: base64URLToBytes(c.id),
      }));
    }

    // Call WebAuthn API
    const credential = await navigator.credentials.create({ publicKey: options });

    if (!credential) throw new Error('Passkey registration cancelled');

    // Format the response for the server
    const credentialJSON = {
      id: credential.id,
      type: credential.type,
      rawId: bytesToBase64URL(new Uint8Array(credential.rawId)),
      response: {
        clientDataJSON: bytesToBase64URL(new Uint8Array(credential.response.clientDataJSON)),
        attestationObject: bytesToBase64URL(new Uint8Array(credential.response.attestationObject)),
        transports: credential.response.getTransports ? credential.response.getTransports() : [],
      },
    };

    // Send to server
    const res = await fetch(`/api/auth/passkey/register/complete`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        challenge: registrationState.options.challenge,
        credential: credentialJSON,
      }),
    });

    if (!res.ok) throw new Error((await res.json()).error);
    const result = await res.json();

    $('register-step2').classList.add('hidden');
    $('register-complete').classList.remove('hidden');
  } catch (err) {
    alert('Passkey registration failed: ' + err.message);
  }
}

// ─── Login Flow ────────────────────────────────────────────────────────────

async function beginLogin() {
  showView('login');
  $('login-error').classList.add('hidden');

  try {
    const res = await fetch(`/api/auth/passkey/login/begin`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });

    if (!res.ok) throw new Error((await res.json()).error);
    const data = await res.json();

    const options = data.options;
    options.challenge = base64URLToBytes(options.challenge);
    if (options.allowCredentials) {
      options.allowCredentials = options.allowCredentials.map(c => ({
        ...c,
        id: base64URLToBytes(c.id),
      }));
    }

    const credential = await navigator.credentials.get({ publicKey: options });
    if (!credential) {
      $('login-error').textContent = 'Authentication cancelled.';
      $('login-error').classList.remove('hidden');
      return;
    }

    const credentialJSON = {
      id: credential.id,
      type: credential.type,
      rawId: bytesToBase64URL(new Uint8Array(credential.rawId)),
      response: {
        clientDataJSON: bytesToBase64URL(new Uint8Array(credential.response.clientDataJSON)),
        authenticatorData: bytesToBase64URL(new Uint8Array(credential.response.authenticatorData)),
        signature: bytesToBase64URL(new Uint8Array(credential.response.signature)),
        userHandle: credential.response.userHandle
          ? bytesToBase64URL(new Uint8Array(credential.response.userHandle))
          : null,
      },
    };

    const verifyRes = await fetch(`/api/auth/passkey/login/complete`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ credential: credentialJSON }),
    });

    if (!verifyRes.ok) throw new Error((await verifyRes.json()).error);
    const result = await verifyRes.json();

    // Store session token
    document.cookie = `mosaic_session=${result.sessionToken}; path=/; max-age=604800; SameSite=Strict`;

    await loadDashboard();
  } catch (err) {
    $('login-error').textContent = 'Login failed: ' + err.message;
    $('login-error').classList.remove('hidden');
  }
}

// ─── Dashboard ─────────────────────────────────────────────────────────────

async function loadDashboard() {
  try {
    const res = await fetch(`/api/auth/identity/current`);
    if (!res.ok) {
      showView('splash');
      return;
    }

    const data = await res.json();
    const id = data.identity;

    $('dash-pubkey').textContent = id.pubkey;
    $('dash-fingerprint').textContent = id.fingerprint;
    $('dash-hex').textContent = id.pubkeyHex;
    $('dash-uri').textContent = id.uri;
    showView('dashboard');

    // Load QR code
    await loadQR(id.pubkey);
    await loadContacts();
  } catch (err) {
    console.error('Dashboard load failed:', err);
    showView('splash');
  }
}

async function loadQR(pubkey) {
  const container = $('qr-container');
  try {
    const res = await fetch(`/mosaic/qr/${encodeURIComponent(pubkey)}`);
    if (!res.ok) throw new Error('QR generation failed');

    const svg = await res.text();
    container.innerHTML = svg;
  } catch (err) {
    container.innerHTML = '<p class="error">QR unavailable</p>';
  }
}

// ─── Contacts ──────────────────────────────────────────────────────────────

async function loadContacts() {
  try {
    const res = await fetch(`/mosaic/contacts`);
    if (!res.ok) return;
    const data = await res.json();
    const list = $('contact-list');
    const count = $('contact-count');

    count.textContent = data.contacts.length;

    if (data.contacts.length === 0) {
      list.innerHTML = '<p class="hint">No contacts yet. Scan a QR code to add one.</p>';
      return;
    }

    list.innerHTML = data.contacts.map(c => `
      <div class="contact-item">
        <div>
          <div class="label">${c.label || 'Unknown'}</div>
          <div class="pubkey">${c.pubkey.slice(0, 24)}…</div>
        </div>
        <button class="remove" data-pubkey="${c.pubkey}">✕</button>
      </div>
    `).join('');

    list.querySelectorAll('.remove').forEach(btn => {
      btn.addEventListener('click', async () => {
        await fetch(`/mosaic/contacts/${encodeURIComponent(btn.dataset.pubkey)}`, { method: 'DELETE' });
        await loadContacts();
      });
    });
  } catch (err) {
    console.error('Failed to load contacts:', err);
  }
}

async function scanQR() {
  const input = $('scan-input');
  const result = $('scan-result');
  const data = input.value.trim();

  if (!data) return;

  result.classList.remove('hidden');

  try {
    const res = await fetch(`/mosaic/qr/scan`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ data }),
    });

    if (!res.ok) {
      const err = await res.json();
      result.innerHTML = `<p class="error">${err.error}</p>`;
      return;
    }

    const r = await res.json();
    result.innerHTML = `<p style="color: var(--green)">✓ Added contact: ${r.fingerprint}</p>`;
    input.value = '';
    await loadContacts();
  } catch (err) {
    result.innerHTML = `<p class="error">${err.message}</p>`;
  }
}

// ─── Signing Demo ──────────────────────────────────────────────────────────

let lastEnvelope = null;

async function signData() {
  const input = $('sign-input');
  const result = $('sign-result');
  const sigEl = $('sign-signature');

  try {
    const data = JSON.parse(input.value);
    const res = await fetch(`/mosaic/sign`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ data }),
    });

    if (!res.ok) throw new Error((await res.json()).error);

    lastEnvelope = await res.json();
    sigEl.textContent = lastEnvelope.signature.slice(0, 48) + '…';
    result.classList.remove('hidden');
    $('verify-result').classList.add('hidden');
  } catch (err) {
    alert('Signing failed: ' + err.message);
  }
}

async function verifyData() {
  if (!lastEnvelope) return;
  const result = $('verify-result');

  try {
    const res = await fetch(`/mosaic/verify`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ envelope: lastEnvelope }),
    });

    const data = await res.json();
    result.classList.remove('hidden');
    result.innerHTML = data.verified
      ? '<p style="color: var(--green)">✓ Signature verified</p>'
      : '<p style="color: var(--red)">✗ Signature invalid</p>';
  } catch (err) {
    result.innerHTML = `<p class="error">${err.message}</p>`;
  }
}

// ─── Identities List ───────────────────────────────────────────────────────

async function showIdentities() {
  showView('existingIdentities');
  try {
    const res = await fetch(`/api/auth/identity`);
    if (!res.ok) throw new Error('Failed to load');
    const data = await res.json();
    const list = $('identities-list');

    if (data.identities.length === 0) {
      list.innerHTML = '<p class="hint">No identities on this device.</p>';
      return;
    }

    list.innerHTML = data.identities.map(id => `
      <div class="contact-item">
        <div>
          <div class="label">${id.label || 'Unnamed'}</div>
          <div class="pubkey">${id.pubkey.slice(0, 32)}…</div>
          <div class="pubkey" style="font-size:0.7rem">Created: ${new Date(id.created_at + 'Z').toLocaleString()}</div>
        </div>
        ${id.is_current ? '<span style="color:var(--green)">✓ current</span>' : ''}
      </div>
    `).join('');
  } catch (err) {
    $('identities-list').innerHTML = `<p class="error">${err.message}</p>`;
  }
}

// ─── Logout ────────────────────────────────────────────────────────────────

async function logout() {
  await fetch(`/api/auth/passkey/logout`, { method: 'POST' });
  document.cookie = 'mosaic_session=; path=/; max-age=0';
  showView('splash');
}

// ─── Init ──────────────────────────────────────────────────────────────────

async function init() {
  // Check connection
  await checkConnection();

  // Check if already authenticated
  try {
    const res = await fetch(`/api/auth/identity/current`);
    if (res.ok) {
      await loadDashboard();
      return;
    }
  } catch { /* not authenticated */ }

  showView('splash');

  // Event listeners
  $('btn-register')?.addEventListener('click', beginRegistration);
  $('btn-register-passkey')?.addEventListener('click', registerPasskey);
  $('btn-reg-go-dashboard')?.addEventListener('click', loadDashboard);

  $('btn-login')?.addEventListener('click', beginLogin);
  $('btn-login-passkey')?.addEventListener('click', beginLogin);

  $('btn-logout')?.addEventListener('click', logout);

  $('btn-show-existing')?.addEventListener('click', showIdentities);
  $('btn-back-splash')?.addEventListener('click', () => showView('splash'));

  $('btn-scan')?.addEventListener('click', scanQR);
  $('scan-input')?.addEventListener('keydown', (e) => { if (e.key === 'Enter') scanQR(); });

  $('btn-sign')?.addEventListener('click', signData);
  $('btn-verify')?.addEventListener('click', verifyData);

  // Register passkey on enter in step 2
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !$('register-step2')?.classList.contains('hidden')) {
      registerPasskey();
    }
  });
}

document.addEventListener('DOMContentLoaded', init);
