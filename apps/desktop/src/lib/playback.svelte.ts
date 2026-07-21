// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `playback` — centralized playback queue state and logic.
//
// The queue is persisted to the SQLite-backed `playback_queue` table so it
// survives app restarts. `queue_index` and `repeat_mode` are stored in
// settings.json alongside other preferences.

import { invoke } from '@tauri-apps/api/core';

/** Repeat / shuffle mode for the playback queue. */
export type RepeatMode = 'sequential' | 'repeat-one' | 'shuffle';

/** A single track in the playback queue. Mirrors the subset of `SongInfo`
 *  needed to call the `play` Tauri command. */
export interface QueueTrack {
	path: string;
	title?: string;
	artist?: string;
	album?: string;
	duration_secs?: number;
	album_art_path?: string;
}

/** Central playback state, reactive via Svelte 5 runes. */
export const playback = $state({
	queue: [] as QueueTrack[],
	queueIndex: -1,
	repeatMode: 'sequential' as RepeatMode,
});

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

/** Immediately persist a partial settings object to the backend. No
 *  debounce — `queue_index` must survive even if the app closes right
 *  after a skip. The backend `set_settings` merges partials (only
 *  overwrites fields that are `Some`), so this is cheap and safe. */
async function persistSettings(partial: Record<string, unknown>): Promise<void> {
	try {
		await invoke('set_settings', { settings: partial });
	} catch (e) {
		console.warn('[playback] persistSettings failed:', e);
	}
}

function persistQueueIndex(): void {
	void persistSettings({ queue_index: playback.queueIndex });
}

function persistRepeatMode(): void {
	void persistSettings({ repeat_mode: playback.repeatMode });
}

async function persistQueueToDb(): Promise<void> {
	try {
		await invoke('set_playback_queue', { tracks: playback.queue });
	} catch (e) {
		console.warn('[playback] persistQueueToDb failed:', e);
	}
}

// ---------------------------------------------------------------------------
// Rehydration — load persisted state on app start
// ---------------------------------------------------------------------------

let rehydrated = false;

/** Load the persisted queue, queue index, and repeat mode from the DB +
 *  settings. Call once on app mount. */
export async function rehydratePlayback(): Promise<void> {
	if (rehydrated) return;
	rehydrated = true;
	try {
		const [dbQueue, settings] = await Promise.all([
			invoke<QueueTrack[]>('get_playback_queue').catch(() => [] as QueueTrack[]),
			invoke<Record<string, unknown>>('get_settings').catch(() => ({}) as Record<string, unknown>),
		]);
		if (Array.isArray(dbQueue) && dbQueue.length > 0) {
			playback.queue = dbQueue;
		}
		const qi = settings?.queue_index;
		if (typeof qi === 'number' && qi >= 0 && qi < playback.queue.length) {
			playback.queueIndex = qi;
		}
		const rm = settings?.repeat_mode as string | undefined;
		if (rm === 'sequential' || rm === 'repeat-one' || rm === 'shuffle') {
			playback.repeatMode = rm;
		}
	} catch {
		// settings / DB not available yet — keep defaults
	}
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/** Cycle: sequential → repeat-one → shuffle → sequential. */
export function cycleRepeatMode(): RepeatMode {
	const order: RepeatMode[] = ['sequential', 'repeat-one', 'shuffle'];
	const idx = order.indexOf(playback.repeatMode);
	playback.repeatMode = order[(idx + 1) % order.length];
	persistRepeatMode();
	return playback.repeatMode;
}

/** Replace the queue and set the active index. Persists to the DB and
 *  settings. The caller is responsible for calling `playQueueIndex` (or
 *  their own `play` invocation) to start playback. */
export async function setQueue(tracks: QueueTrack[], startIndex: number): Promise<void> {
	playback.queue = tracks;
	playback.queueIndex = startIndex;
	await Promise.all([
		persistQueueToDb(),
		persistSettings({ queue_index: startIndex }),
	]);
}

/** Probe a file for sample rate / bit depth before playback. */
async function probeSong(
	path: string,
): Promise<{ sample_rate?: number; bits_per_sample?: number; duration_secs?: number }> {
	try {
		const info = await invoke<{ sample_rate: number; channels: number; duration_secs: number; bits_per_sample?: number }>(
			'probe_audio_file',
			{ path },
		);
		return {
			sample_rate: info.sample_rate,
			bits_per_sample: info.bits_per_sample,
			duration_secs: info.duration_secs,
		};
	} catch {
		return {};
	}
}

/** Play the track at the given queue index. Fetches metadata (title,
 *  artist, album, album art) so the backend's `CURRENT_SONG_INFO` is always
 *  fully populated — without this the Now Playing page shows blank
 *  album/artist info for queue-advanced tracks. */
export async function playQueueIndex(index: number): Promise<void> {
	const track = playback.queue[index];
	if (!track) return;
	playback.queueIndex = index;
	persistQueueIndex();

	let meta: { title?: string; artist?: string; album?: string } = {};
	try {
		meta = await invoke<{ title?: string; artist?: string; album?: string }>(
			'get_song_metadata',
			{ path: track.path },
		);
	} catch { /* best-effort */ }
	let artPath = track.album_art_path;
	if (!artPath) {
		try {
			artPath = await invoke<string | null>('get_album_art_path', { audioPath: track.path }) ?? undefined;
		} catch { /* best-effort */ }
	}

	if (meta.title) track.title = meta.title;
	if (meta.artist) track.artist = meta.artist;
	if (meta.album) track.album = meta.album;
	if (artPath) track.album_art_path = artPath;

	const probe = await probeSong(track.path);
	await invoke('play', {
		song: {
			path: track.path,
			title: meta.title || track.title,
			artist: meta.artist || track.artist,
			album: meta.album || track.album,
			album_art_path: artPath,
			sample_rate: probe.sample_rate,
			bits_per_sample: probe.bits_per_sample,
			duration_secs: probe.duration_secs ?? track.duration_secs,
		},
	});
}

/** Pick a random index in [0, len), preferring one different from `avoid`. */
function randomIndex(len: number, avoid: number): number {
	if (len <= 0) return -1;
	if (len === 1) return 0;
	let idx = avoid;
	let tries = 0;
	while (idx === avoid && tries < 12) {
		idx = Math.floor(Math.random() * len);
		tries++;
	}
	return idx;
}

/** Manual skip to the next track. */
export async function goNext(): Promise<void> {
	const len = playback.queue.length;
	if (len === 0) return;
	if (playback.repeatMode === 'shuffle') {
		const idx = randomIndex(len, playback.queueIndex);
		if (idx >= 0) await playQueueIndex(idx);
		return;
	}
	const next = playback.queueIndex + 1;
	if (next < len) {
		await playQueueIndex(next);
	} else if (playback.repeatMode === 'repeat-one') {
		await playQueueIndex(0);
	}
}

/** Manual skip to the previous track. */
export async function goPrev(): Promise<void> {
	const len = playback.queue.length;
	if (len === 0) return;
	if (playback.repeatMode === 'shuffle') {
		const idx = randomIndex(len, playback.queueIndex);
		if (idx >= 0) await playQueueIndex(idx);
		return;
	}
	const prev = playback.queueIndex - 1;
	if (prev >= 0) {
		await playQueueIndex(prev);
	} else if (playback.repeatMode === 'repeat-one') {
		await playQueueIndex(len - 1);
	}
}

/** Auto-advance handler — called when the engine emits `track_ended`. */
export async function onTrackEnded(): Promise<void> {
	const len = playback.queue.length;
	if (len === 0) return;

	if (playback.repeatMode === 'repeat-one') {
		await playQueueIndex(playback.queueIndex);
		return;
	}

	if (playback.repeatMode === 'shuffle') {
		const idx = randomIndex(len, playback.queueIndex);
		if (idx >= 0) await playQueueIndex(idx);
		return;
	}

	// sequential
	const next = playback.queueIndex + 1;
	if (next < len) {
		await playQueueIndex(next);
	}
}

/** The currently-active track, or `null` if the queue is empty. */
export function currentTrack(): QueueTrack | null {
	return playback.queueIndex >= 0 ? playback.queue[playback.queueIndex] ?? null : null;
}

/** Sync `queueIndex` to the track matching `path` in the queue. Used by
 *  Now Playing's `togglePlay` when it plays the persisted last-played
 *  track directly (not through `playQueueIndex`) — without this the
 *  index would be stale and skip-next would jump to the wrong position.
 *  Returns `true` if the path was found in the queue. */
export function syncQueueIndexByPath(path: string): boolean {
	const idx = playback.queue.findIndex((t) => t.path === path);
	if (idx >= 0) {
		playback.queueIndex = idx;
		persistQueueIndex();
		return true;
	}
	return false;
}