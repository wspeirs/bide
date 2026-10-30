import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import tailwindcss from '@tailwindcss/vite';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    tailwindcss(),
    vue(),
  ],
  // Prevent vite from obscuring rust errors
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  test: {
    environment: 'jsdom',
  },
});
