/**
 * Thin fetch wrapper for the Paddock Manager API.
 * Every helper throws an Error with a readable message on failure.
 */

async function request(method, url, body, opts = {}) {
  const init = { method, headers: {} };
  if (body !== undefined && body !== null) {
    // Let the browser set the multipart boundary for FormData bodies.
    if (typeof FormData !== 'undefined' && body instanceof FormData) {
      init.body = body;
    } else {
      init.headers['Content-Type'] = 'application/json';
      init.body = JSON.stringify(body);
    }
  }
  const res = await fetch(url, init);
  if (res.status === 204) return null;

  const type = res.headers.get('content-type') || '';
  const payload = type.includes('application/json') ? await res.json() : await res.text();

  if (!res.ok) {
    const message = (payload && payload.error) || `${method} ${url} failed (${res.status})`;
    const err = new Error(message);
    err.status = res.status;
    err.payload = payload;
    throw err;
  }
  return payload;
}

export const api = {
  get: (url) => request('GET', url),
  post: (url, body) => request('POST', url, body),
  put: (url, body) => request('PUT', url, body),
  del: (url) => request('DELETE', url),
};

/** Download helper that triggers a browser save for the PDF endpoints. */
export function download(url, filename) {
  const a = document.createElement('a');
  a.href = url;
  if (filename) a.download = filename;
  a.rel = 'noopener';
  document.body.appendChild(a);
  a.click();
  a.remove();
}

export const links = {
  async pubcris(q) {
    return (await api.get(`/api/links/pubcris?q=${encodeURIComponent(q || '')}`)).url;
  },
  async bom(q) {
    return (await api.get(`/api/links/bom?q=${encodeURIComponent(q || '')}`)).url;
  },
};