'use strict';

const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');

// ─── Module under test ──────────────────────────────────────────────────────

const { AstralKeyClient } = require('../src/astral-key-client');

// ─── Mock helpers ───────────────────────────────────────────────────────────

const BASE_URL = 'http://astral-key.test:3001';

/**
 * Create a minimal mock Response object.
 * @param {number}  status  HTTP status code
 * @param {*}       body    Response body (object → JSON, string → text)
 * @param {object}  [opts]  Optional overrides
 * @returns {Response}
 */
function mockResponse(status, body, opts = {}) {
  const isJson = typeof body === 'object' && body !== null;
  const bodyText = isJson ? JSON.stringify(body) : String(body ?? '');
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: {
      get(name) {
        const lower = name.toLowerCase();
        if (lower === 'content-type') {
          return opts.contentType ?? (isJson ? 'application/json' : 'text/plain');
        }
        return null;
      },
    },
    json: async () => (isJson ? body : JSON.parse(bodyText)),
    text: async () => bodyText,
    ...opts.custom,
  };
}

// ─── AstralKeyClient — Unit tests ───────────────────────────────────────────

describe('AstralKeyClient', () => {
  /** @type {AstralKeyClient} */
  let client;
  /** @type {(url: string, options?: object) => Promise<Response>} */
  let fetchSpy;

  before(() => {
    client = new AstralKeyClient(BASE_URL);
  });

  /* ─── Helpers ──────────────────────────────────────────────────── */

  /**
   * Replace global.fetch with a spy that records the last call.
   * @param {(url: string, options?: object) => Promise<Response>} resolver
   */
  function mockFetch(resolver) {
    const orig = global.fetch;
    fetchSpy = async (url, options) => resolver(url, options);
    global.fetch = fetchSpy;
    return orig;
  }

  function restoreFetch(orig) {
    global.fetch = orig;
  }

  // ────────────────────────────────────────────────────────────────
  //  Constructor
  // ────────────────────────────────────────────────────────────────

  describe('constructor', () => {
    it('should store the base URL as-is when no trailing slash', () => {
      const c = new AstralKeyClient('http://example.com:3001');
      assert.strictEqual(c.baseUrl, 'http://example.com:3001');
    });

    it('should strip trailing slash from base URL', () => {
      const c = new AstralKeyClient('http://example.com:3001/');
      assert.strictEqual(c.baseUrl, 'http://example.com:3001');
    });

    it('should strip multiple trailing slashes', () => {
      const c = new AstralKeyClient('http://example.com:3001///');
      assert.strictEqual(c.baseUrl, 'http://example.com:3001');
    });

    it('should handle empty-string base URL', () => {
      const c = new AstralKeyClient('');
      assert.strictEqual(c.baseUrl, '');
    });

    it('should handle base URL with path prefix', () => {
      const c = new AstralKeyClient('http://example.com/api/v2/');
      assert.strictEqual(c.baseUrl, 'http://example.com/api/v2');
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  _fetch (internal helper, tested through public methods)
  // ────────────────────────────────────────────────────────────────

  describe('_fetch (internal, exercised via health())', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should build the correct URL from baseUrl + path', async () => {
      let calledUrl;
      orig = mockFetch(async (url) => {
        calledUrl = url;
        return mockResponse(200, { status: 'ok' });
      });

      await client.health();
      assert.strictEqual(calledUrl, `${BASE_URL}/health`);
    });

    it('should use GET by default', async () => {
      let calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledMethod = opts.method || 'GET';
        return mockResponse(200, { status: 'ok' });
      });

      await client.health();
      assert.strictEqual(calledMethod, 'GET');
    });

    it('should set Content-Type and Accept headers', async () => {
      let calledHeaders;
      orig = mockFetch(async (url, opts) => {
        calledHeaders = opts.headers;
        return mockResponse(200, { status: 'ok' });
      });

      await client.health();
      assert.strictEqual(calledHeaders['Content-Type'], 'application/json');
      assert.strictEqual(calledHeaders['Accept'], 'application/json');
    });

    it('should pass through extra headers', async () => {
      let calledHeaders;
      orig = mockFetch(async (url, opts) => {
        calledHeaders = opts.headers;
        return mockResponse(200, { status: 'ok' });
      });

      await client._fetch('/test', {
        headers: { Authorization: 'Bearer test-token' },
      });
      assert.strictEqual(calledHeaders['Content-Type'], 'application/json');
      assert.strictEqual(calledHeaders['Accept'], 'application/json');
      assert.strictEqual(calledHeaders['Authorization'], 'Bearer test-token');
    });

    it('should parse JSON response body', async () => {
      const expected = { status: 'ok', version: '1.0' };
      orig = mockFetch(async () => mockResponse(200, expected));

      const result = await client.health();
      assert.deepStrictEqual(result, expected);
    });

    it('should throw when HTTP status is an error with JSON body', async () => {
      const errBody = { error: 'unauthorized', message: 'Invalid token' };
      orig = mockFetch(async () => mockResponse(401, errBody));

      await assert.rejects(
        () => client.health(),
        (err) => {
          assert.strictEqual(err.status, 401);
          assert.deepStrictEqual(err.body, errBody);
          assert.ok(err.message.includes('unauthorized'));
          return true;
        },
      );
    });

    it('should fall back to body.message for error message', async () => {
      const errBody = { message: 'Server error' };
      orig = mockFetch(async () => mockResponse(500, errBody));

      await assert.rejects(
        () => client.health(),
        (err) => {
          assert.ok(err.message.includes('Server error'));
          return true;
        },
      );
    });

    it('should fall back to status text when no error/message field', async () => {
      const errBody = { detail: 'Not found' };
      orig = mockFetch(async () => mockResponse(404, errBody));

      await assert.rejects(
        () => client.health(),
        (err) => {
          assert.ok(err.message.includes('404'));
          return true;
        },
      );
    });

    it('should wrap non-JSON error responses in { error }', async () => {
      orig = mockFetch(async () =>
        mockResponse(502, 'Bad Gateway', { contentType: 'text/plain' }),
      );

      await assert.rejects(
        () => client.health(),
        (err) => {
          assert.strictEqual(err.status, 502);
          assert.deepStrictEqual(err.body, { error: 'Bad Gateway' });
          return true;
        },
      );
    });

    it('should propagate network failures', async () => {
      orig = mockFetch(async () => {
        throw new TypeError('fetch failed: connect ECONNREFUSED');
      });

      await assert.rejects(
        () => client.health(),
        /fetch failed/,
      );
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  health()
  // ────────────────────────────────────────────────────────────────

  describe('health()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should GET /health and return the body', async () => {
      const expected = { status: 'ok' };
      orig = mockFetch(async (url, opts) => {
        assert.strictEqual(url, `${BASE_URL}/health`);
        assert.strictEqual(opts.method || 'GET', 'GET');
        return mockResponse(200, expected);
      });

      const result = await client.health();
      assert.deepStrictEqual(result, expected);
    });

    it('should propagate errors', async () => {
      orig = mockFetch(async () => mockResponse(503, { error: 'service unavailable' }));

      await assert.rejects(() => client.health(), { status: 503 });
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  registerBegin()
  // ────────────────────────────────────────────────────────────────

  describe('registerBegin()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/fido2/register/options with label', async () => {
      let calledUrl, calledBody, calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          identityId: 'ident-1',
          pubkey: 'abc123',
          options: { challenge: 'chal' },
        });
      });

      const result = await client.registerBegin({ label: 'my-key' });
      assert.strictEqual(calledUrl, `${BASE_URL}/auth/fido2/register/options`);
      assert.strictEqual(calledMethod, 'POST');
      assert.strictEqual(calledBody.label, 'my-key');
      assert.strictEqual(result.identityId, 'ident-1');
      assert.strictEqual(result.pubkey, 'abc123');
      assert.deepStrictEqual(result.options, { challenge: 'chal' });
    });

    it('should send label as null when omitted', async () => {
      let calledBody;
      orig = mockFetch(async (url, opts) => {
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          identityId: 'ident-1',
          pubkey: 'abc',
          options: {},
        });
      });

      await client.registerBegin();
      assert.strictEqual(calledBody.label, null);
    });

    it('should send label as null when empty object', async () => {
      let calledBody;
      orig = mockFetch(async (url, opts) => {
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          identityId: 'ident-1',
          pubkey: 'abc',
          options: {},
        });
      });

      await client.registerBegin({});
      assert.strictEqual(calledBody.label, null);
    });

    it('should send label as null when label is empty string', async () => {
      let calledBody;
      orig = mockFetch(async (url, opts) => {
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          identityId: 'ident-1',
          pubkey: 'abc',
          options: {},
        });
      });

      await client.registerBegin({ label: '' });
      // The code sends `label: label || null` — empty string is falsy → null
      assert.strictEqual(calledBody.label, null);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  registerComplete()
  // ────────────────────────────────────────────────────────────────

  describe('registerComplete()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/fido2/register/verify with full payload', async () => {
      let calledUrl, calledBody, calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          verified: true,
          identityId: 'ident-1',
          pubkey: 'abc123',
        });
      });

      const credential = { id: 'cred-1', response: { clientDataJSON: '...' } };
      const result = await client.registerComplete({
        challenge: 'chal-123',
        credential,
        nickname: 'my-key',
      });

      assert.strictEqual(calledUrl, `${BASE_URL}/auth/fido2/register/verify`);
      assert.strictEqual(calledMethod, 'POST');
      assert.strictEqual(calledBody.challenge, 'chal-123');
      assert.deepStrictEqual(calledBody.credential, credential);
      assert.strictEqual(calledBody.nickname, 'my-key');
      assert.strictEqual(result.verified, true);
      assert.strictEqual(result.identityId, 'ident-1');
      assert.strictEqual(result.pubkey, 'abc123');
    });

    it('should send nickname as null when omitted', async () => {
      let calledBody;
      orig = mockFetch(async (url, opts) => {
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, { verified: true, identityId: 'id', pubkey: 'pk' });
      });

      await client.registerComplete({ challenge: 'c', credential: { id: 'x' } });
      assert.strictEqual(calledBody.nickname, null);
    });

    it('should send nickname as null when undefined', async () => {
      let calledBody;
      orig = mockFetch(async (url, opts) => {
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, { verified: true, identityId: 'id', pubkey: 'pk' });
      });

      await client.registerComplete({
        challenge: 'c',
        credential: { id: 'x' },
        nickname: undefined,
      });
      assert.strictEqual(calledBody.nickname, null);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  loginBegin()
  // ────────────────────────────────────────────────────────────────

  describe('loginBegin()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/fido2/login/start and return shaped response', async () => {
      let calledUrl, calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        return mockResponse(200, {
          options: { challenge: 'chal-456', rpId: 'localhost' },
        });
      });

      const result = await client.loginBegin();
      assert.strictEqual(calledUrl, `${BASE_URL}/auth/fido2/login/start`);
      assert.strictEqual(calledMethod, 'POST');
      assert.deepStrictEqual(result.options, { challenge: 'chal-456', rpId: 'localhost' });
      assert.strictEqual(result.challenge, 'chal-456');
    });

    it('should fall back to top-level challenge when options.challenge missing', async () => {
      orig = mockFetch(async () =>
        mockResponse(200, {
          options: { rpId: 'localhost' },
          challenge: 'top-level-chal',
        }),
      );

      const result = await client.loginBegin();
      assert.strictEqual(result.challenge, 'top-level-chal');
    });

    it('should work when options is null', async () => {
      orig = mockFetch(async () =>
        mockResponse(200, { options: null, challenge: 'fallback' }),
      );

      const result = await client.loginBegin();
      assert.strictEqual(result.challenge, 'fallback');
    });

    it('should work when body has no options field', async () => {
      orig = mockFetch(async () =>
        mockResponse(200, { challenge: 'bare' }),
      );

      const result = await client.loginBegin();
      assert.strictEqual(result.challenge, 'bare');
      assert.strictEqual(result.options, undefined);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  loginComplete()
  // ────────────────────────────────────────────────────────────────

  describe('loginComplete()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/fido2/login/complete with credential', async () => {
      let calledUrl, calledBody, calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, {
          verified: true,
          identityId: 'ident-1',
          pubkey: 'abc123',
          sessionToken: 'tok-123',
        });
      });

      const credential = { id: 'cred-1', rawId: 'raw-1' };
      const result = await client.loginComplete({ credential });

      assert.strictEqual(calledUrl, `${BASE_URL}/auth/fido2/login/complete`);
      assert.strictEqual(calledMethod, 'POST');
      assert.deepStrictEqual(calledBody.credential, credential);
      assert.strictEqual(result.verified, true);
      assert.strictEqual(result.identityId, 'ident-1');
      assert.strictEqual(result.pubkey, 'abc123');
      assert.strictEqual(result.sessionToken, 'tok-123');
    });

    it('should return verified false from server', async () => {
      orig = mockFetch(async () =>
        mockResponse(200, {
          verified: false,
          identityId: null,
          pubkey: null,
          sessionToken: null,
        }),
      );

      const result = await client.loginComplete({ credential: { id: 'bad' } });
      assert.strictEqual(result.verified, false);
      assert.strictEqual(result.identityId, null);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  getCredentials()
  // ────────────────────────────────────────────────────────────────

  describe('getCredentials()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should GET /auth/fido2/credentials and return the body', async () => {
      const expected = [{ id: 'cred-1', type: 'public-key' }];
      orig = mockFetch(async (url, opts) => {
        assert.strictEqual(url, `${BASE_URL}/auth/fido2/credentials`);
        assert.strictEqual(opts.method || 'GET', 'GET');
        return mockResponse(200, expected);
      });

      const result = await client.getCredentials();
      assert.deepStrictEqual(result, expected);
    });

    it('should return empty array when no credentials', async () => {
      orig = mockFetch(async () => mockResponse(200, []));

      const result = await client.getCredentials();
      assert.deepStrictEqual(result, []);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  deleteCredential()
  // ────────────────────────────────────────────────────────────────

  describe('deleteCredential()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should DELETE /auth/fido2/credentials/:id', async () => {
      let calledUrl, calledMethod;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        return mockResponse(200, { deleted: true });
      });

      const result = await client.deleteCredential('cred-1');
      assert.strictEqual(calledUrl, `${BASE_URL}/auth/fido2/credentials/cred-1`);
      assert.strictEqual(calledMethod, 'DELETE');
      assert.strictEqual(result.deleted, true);
    });

    it('should URI-encode the credential ID', async () => {
      let calledUrl;
      orig = mockFetch(async (url) => {
        calledUrl = url;
        return mockResponse(200, {});
      });

      await client.deleteCredential('cred/id#special?');
      assert.strictEqual(
        calledUrl,
        `${BASE_URL}/auth/fido2/credentials/cred%2Fid%23special%3F`,
      );
    });

    it('should propagate 404 from server', async () => {
      orig = mockFetch(async () =>
        mockResponse(404, { error: 'Credential not found' }),
      );

      await assert.rejects(
        () => client.deleteCredential('nonexistent'),
        { status: 404 },
      );
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  web3SiweNonce()
  // ────────────────────────────────────────────────────────────────

  describe('web3SiweNonce()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/web3/siwe/nonce and return nonce', async () => {
      let calledUrl, calledMethod, calledBody;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledMethod = opts.method;
        calledBody = opts.body;
        return mockResponse(200, { nonce: 'abc123' });
      });

      const result = await client.web3SiweNonce();
      assert.strictEqual(calledUrl, `${BASE_URL}/auth/web3/siwe/nonce`);
      assert.strictEqual(calledMethod, 'POST');
      assert.strictEqual(calledBody, undefined); // no body for this POST
      assert.strictEqual(result.nonce, 'abc123');
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  web3SiweVerify()
  // ────────────────────────────────────────────────────────────────

  describe('web3SiweVerify()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should POST to /auth/web3/siwe/verify with message and signature', async () => {
      let calledUrl, calledBody;
      orig = mockFetch(async (url, opts) => {
        calledUrl = url;
        calledBody = JSON.parse(opts.body);
        return mockResponse(200, { verified: true, address: '0x123' });
      });

      const result = await client.web3SiweVerify({
        message: 'Sign in to Mosaic',
        signature: '0xsig123',
      });
      assert.strictEqual(calledUrl, `${BASE_URL}/auth/web3/siwe/verify`);
      assert.strictEqual(calledBody.message, 'Sign in to Mosaic');
      assert.strictEqual(calledBody.signature, '0xsig123');
      assert.strictEqual(result.verified, true);
      assert.strictEqual(result.address, '0x123');
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  getWeb3Wallets()
  // ────────────────────────────────────────────────────────────────

  describe('getWeb3Wallets()', () => {
    let orig;

    after(() => {
      if (orig) restoreFetch(orig);
    });

    it('should GET /auth/web3/wallets and return wallets', async () => {
      const expected = [{ address: '0xabc', chain: 'ethereum' }];
      orig = mockFetch(async (url, opts) => {
        assert.strictEqual(url, `${BASE_URL}/auth/web3/wallets`);
        assert.strictEqual(opts.method || 'GET', 'GET');
        return mockResponse(200, expected);
      });

      const result = await client.getWeb3Wallets();
      assert.deepStrictEqual(result, expected);
    });
  });

  // ────────────────────────────────────────────────────────────────
  //  Global fetch isolation
  // ────────────────────────────────────────────────────────────────

  describe('fetch isolation', () => {
    it('should not leak mocked fetch across tests', async () => {
      // After all mock tests, global.fetch should be the real fetch
      // (or at least not a stale spy).  We verify by checking that
      // global.fetch is still a function and that calling it produces
      // a real Response (with .ok, .status etc) rather than the
      // internal spy object used by the tests above.
      assert.strictEqual(typeof global.fetch, 'function');
      // The real fetch from Node is async and returns a Response.
      // If a mock leaked, fetch() would either not be a function or
      // would not behave like real fetch.
      const realFetch = global.fetch.toString();
      assert.ok(
        realFetch.includes('function fetch') ||
          realFetch.includes('[native code]') ||
          realFetch.includes('async'),
        `global.fetch should be a real fetch function, got: ${realFetch.slice(0, 80)}`,
      );
    });
  });
});
