export interface ClientCredentials {
  id: string;
  secret: string;
}

export function readClientJson(text: string): ClientCredentials | null {
  try {
    const parsed = JSON.parse(text) as Record<string, unknown>;
    const body = (parsed.installed ?? parsed.web ?? parsed) as Record<string, unknown>;
    const id = typeof body.client_id === 'string' ? body.client_id.trim() : '';
    const secret = typeof body.client_secret === 'string' ? body.client_secret.trim() : '';
    return id && secret ? { id, secret } : null;
  } catch {
    return null;
  }
}

export function clientIdProblem(id: string): string | null {
  const value = id.trim();
  if (!value) return null;
  if (/\s/.test(value)) return 'The client ID has no spaces. Copy it again.';
  if (!value.endsWith('.apps.googleusercontent.com'))
    return 'A client ID ends in .apps.googleusercontent.com. Check you copied the ID, not the secret.';
  return null;
}
