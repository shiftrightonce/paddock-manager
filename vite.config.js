import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

const API_TARGET = process.env.VITE_API_TARGET || 'http://localhost:3000';

export default defineConfig({
  plugins: [vue()],
  root: 'web',
  publicDir: '../public',
  server: {
    port: 5173,
    proxy: {
      '/api': { target: API_TARGET, changeOrigin: true },
      '/uploads': { target: API_TARGET, changeOrigin: true },
    },
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    sourcemap: false,
  },
});