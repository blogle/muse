import { defineConfig } from 'vite';

export default defineConfig({
  publicDir: '../../fixtures',
  server: { fs: { allow: ['../..'] } },
});
