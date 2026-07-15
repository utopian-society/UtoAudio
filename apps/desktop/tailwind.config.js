/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{svelte,js,ts}',
    '!./src/lib/vendor/**',
  ],
  darkMode: 'class',
  theme: {
    extend: {},
  },
  plugins: [],
}
