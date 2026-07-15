import { invoke } from '@tauri-apps/api/core';
import { AUDIO_EXTENSIONS } from './file-browser';

export const appState = $state({
  enabledExtensions: new Set<string>(AUDIO_EXTENSIONS),
  lyricFontSize: 36,
  equalizer: null as EqualizerSettings | null,
  crossfade: null as CrossfadeSettings | null,
  convolver: null as ConvolverSettings | null,
});

interface EqualizerSettings {
  enabled: boolean;
  bands: { freq_hz: number; gain_db: number }[];
}
interface CrossfadeSettings {
  enabled: boolean;
  duration_secs: number;
  curve: string;
}
interface ConvolverSettings {
  enabled: boolean;
  mix: number;
}

let rehydrated = false;

export async function rehydrateSettings(): Promise<void> {
  if (rehydrated) return;
  rehydrated = true;
  try {
    const s = await invoke<Record<string, unknown>>('get_settings');
    if (!s) return;
    if (Array.isArray(s.enabled_extensions) && s.enabled_extensions.length > 0) {
      appState.enabledExtensions = new Set(s.enabled_extensions as string[]);
    }
    if (typeof s.lyric_font_size === 'number') {
      appState.lyricFontSize = s.lyric_font_size as number;
    }
    if (s.equalizer) appState.equalizer = s.equalizer as EqualizerSettings;
    if (s.crossfade) appState.crossfade = s.crossfade as CrossfadeSettings;
    if (s.convolver) appState.convolver = s.convolver as ConvolverSettings;
  } catch (e) {
    console.warn('[store] rehydrateSettings failed:', e);
  }
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleSave(): void {
  if (saveTimer != null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void persistSettings();
  }, 500);
}

async function persistSettings(): Promise<void> {
  try {
    await invoke('set_settings', {
      settings: {
        enabled_extensions: Array.from(appState.enabledExtensions),
        lyric_font_size: appState.lyricFontSize,
        equalizer: appState.equalizer,
        crossfade: appState.crossfade,
        convolver: appState.convolver,
      },
    });
  } catch (e) {
    console.warn('[store] persistSettings failed:', e);
  }
}

export function toggleExtension(ext: string): void {
  const next = new Set(appState.enabledExtensions);
  if (next.has(ext)) next.delete(ext);
  else next.add(ext);
  appState.enabledExtensions = next;
  scheduleSave();
}

export function isExtensionEnabled(ext: string): boolean {
  return appState.enabledExtensions.has(ext);
}

export function setLyricFontSize(size: number): void {
  appState.lyricFontSize = size;
  scheduleSave();
}

export function setEqualizer(eq: EqualizerSettings | null): void {
  appState.equalizer = eq;
  scheduleSave();
}

export function setCrossfade(cf: CrossfadeSettings | null): void {
  appState.crossfade = cf;
  scheduleSave();
}

export function setConvolver(cv: ConvolverSettings | null): void {
  appState.convolver = cv;
  scheduleSave();
}
