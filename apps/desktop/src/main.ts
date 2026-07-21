import { mount } from 'svelte';
import './app.css';
import './lib/vendor/amll/packages/core/dist/lyric-player.css';
import App from './App.svelte';

const app = mount(App, {
  target: document.getElementById('app')!,
});

export default app;
