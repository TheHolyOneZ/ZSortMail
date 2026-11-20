import { spawn } from 'node:child_process';

const extra = process.argv.slice(2);
const children = [
  spawn('cargo', ['run', '-q', '-p', 'zsm-devserver', '--', '--demo', ...extra], {
    stdio: 'inherit',
  }),
  spawn('pnpm', ['dev:vite'], { stdio: 'inherit' }),
];

let stopping = false;
function stop(code) {
  if (stopping) return;
  stopping = true;
  for (const child of children) child.kill('SIGTERM');
  process.exit(code);
}

for (const child of children) child.on('exit', (code) => stop(code ?? 0));
process.on('SIGINT', () => stop(0));
process.on('SIGTERM', () => stop(0));
