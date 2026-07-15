import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Tauri-compatible Vite config: fixed dev port (1420), ignore the Rust shell,
// expose Tauri env vars, and allow the monorepo root on the FS layer.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  server: {
    host: '0.0.0.0',
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    fs: {
      allow: ['..'],
    },
  },
  build: {
    target: 'es2021',
  },
})
