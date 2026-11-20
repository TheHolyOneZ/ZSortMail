import { describe, expect, it } from 'vitest';
import { clientIdProblem, readClientJson } from './googleClient';

describe('Google client', () => {
  it('reads the downloaded file', () => {
    const file = JSON.stringify({
      installed: {
        client_id: ' 123.apps.googleusercontent.com ',
        client_secret: 'GOCSPX-abc',
        project_id: 'x',
      },
    });
    expect(readClientJson(file)).toEqual({
      id: '123.apps.googleusercontent.com',
      secret: 'GOCSPX-abc',
    });
    expect(readClientJson('{"web":{"client_id":"a","client_secret":"b"}}')).toEqual({
      id: 'a',
      secret: 'b',
    });
  });

  it('refuses anything else', () => {
    expect(readClientJson('not json')).toBeNull();
    expect(readClientJson('{"installed":{"client_id":"a"}}')).toBeNull();
    expect(readClientJson('[]')).toBeNull();
  });

  it('flags an ID that does not look right', () => {
    expect(clientIdProblem('')).toBeNull();
    expect(clientIdProblem('123.apps.googleusercontent.com')).toBeNull();
    expect(clientIdProblem('GOCSPX-secret')).toMatch(/ends in/);
    expect(clientIdProblem('12 3.apps.googleusercontent.com')).toMatch(/no spaces/);
  });
});
