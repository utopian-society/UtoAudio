<script lang="ts">
	import '../lib/fonts/source-han-sans.css';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import type { UnlistenFn } from '@tauri-apps/api/event';
	import { LyricPlayer, FluidBackground } from '../components/lyrics';
	import { extractTheme } from '../components/lyrics/color';
import { parseLyrics } from '../lib/lyric-parser';
import type { LyricLine, LyricTheme } from '../lib/types/lyrics';
	import Icon from '../components/Icon.svelte';
	import { appState } from '../lib/store.svelte';
	import {
		playback,
		rehydratePlayback,
		cycleRepeatMode,
		playQueueIndex,
		goNext,
		goPrev,
		onTrackEnded,
		syncQueueIndexByPath,
		type RepeatMode,
	} from '../lib/playback.svelte';

	type PlaybackState =
		| 'idle' | 'playing' | 'paused' | 'buffering' | 'crossfading' | 'stopped';

	interface SongInfo {
		path: string;
		title?: string;
		artist?: string;
		album?: string;
		duration_secs?: number;
		album_art_path?: string;
	}

	interface PlaybackProgressInfo {
		position_secs: number;
		duration_secs?: number;
		buffer_level: number;
	}

	type AudioEventInfo =
		| { kind: 'state_changed'; state: PlaybackState }
		| { kind: 'progress'; progress: PlaybackProgressInfo }
		| { kind: 'track_ended'; path: string }
		| { kind: 'crossfade_started'; from_path: string; to_path: string }
		| { kind: 'error'; message: string }
		| { kind: 'next_track_ready'; path: string };

	interface LastPlayedTrack {
		path: string;
		title?: string;
		artist?: string;
		album?: string;
		duration_secs?: number;
		album_art_path?: string;
	}

	let currentTime = $state(0);
	let duration = $state<number | null>(null);
	let playing = $state(false);
	let isSeeking = $state(false);
	let currentTrack = $state<SongInfo | null>(null);
	let lyricLines = $state<LyricLine[]>([]);
	let albumArtBlobUrl = $state('');
	let theme = $state<LyricTheme | undefined>(undefined);
	let showQueue = $state(false);

	// Restore the persisted queue + repeat mode once on mount.
	rehydratePlayback();

	async function loadAlbumArt(artPath: string | undefined): Promise<void> {
		console.log('[NowPlaying] loadAlbumArt: artPath=', artPath);
		if (!artPath) {
			albumArtBlobUrl = '';
			return;
		}
		try {
			const data = await invoke<number[]>('get_album_art_data', { path: artPath });
			const bytes = new Uint8Array(data);
			const blob = new Blob([bytes]);
			albumArtBlobUrl = URL.createObjectURL(blob);
			console.log('[NowPlaying] album art loaded, blob url:', albumArtBlobUrl);
		} catch (e) {
			console.warn('[NowPlaying] get_album_art_data failed:', e);
			albumArtBlobUrl = '';
		}
	}

	async function loadLyrics(audioPath: string): Promise<void> {
		try {
			const payload = await invoke<{ format: string; content: string } | null>('load_lyrics', { path: audioPath });
			if (!payload) {
				lyricLines = [];
				return;
			}
			const lines = await parseLyrics(payload.content, payload.format as 'ttml' | 'yrc' | 'qrc' | 'lrc');
			lyricLines = lines;
		} catch {
			lyricLines = [];
		}
	}

	async function refreshCurrentTrack(): Promise<void> {
		try {
			const info = await invoke<SongInfo | null>('get_current_song_info');
			console.log('[NowPlaying] get_current_song_info:', info);
			if (info) {
				currentTrack = info;
				loadAlbumArt(info.album_art_path);
				loadLyrics(info.path);
			}
		} catch (e) {
			console.warn('[NowPlaying] get_current_song_info failed:', e);
		}
	}

	async function promoteTrack(path: string): Promise<void> {
		try {
			await invoke('set_current_song', { path });
			await refreshCurrentTrack();
		} catch (e) {
			console.warn('[NowPlaying] set_current_song failed:', e);
		}
	}

	// ---------------------------------------------------------------------------
	// Engine wiring: event stream + 2 s poll
	// ---------------------------------------------------------------------------

	$effect(() => {
		let unlistenPromise: Promise<UnlistenFn> | null = null;
		let pollId: ReturnType<typeof setInterval> | null = null;
		let mounted = true;

		invoke('start_event_stream').catch((e) =>
			console.error('[NowPlaying] start_event_stream failed:', e),
		);

		unlistenPromise = listen<AudioEventInfo>('audio-event', (ev) => {
			if (!mounted) return;
			const event = ev.payload;
			switch (event.kind) {
case 'state_changed':
				playing = event.state === 'playing' || event.state === 'crossfading';
				if (event.state === 'playing') refreshCurrentTrack();
				break;
case 'progress':
				if (!isSeeking) currentTime = event.progress.position_secs;
				if (event.progress.duration_secs) duration = event.progress.duration_secs;
				// Race-condition guard: if the engine is playing but currentTrack
				// is stale or missing (e.g. state_changed arrived before
				// CURRENT_SONG_INFO was written), refresh on the first progress tick.
				if (!currentTrack) refreshCurrentTrack();
				break;
				case 'track_ended':
					playing = false;
					void onTrackEnded();
					break;
				case 'crossfade_started':
					promoteTrack(event.to_path);
					break;
				case 'next_track_ready':
					promoteTrack(event.path);
					break;
				case 'error':
					console.warn('[NowPlaying] engine error:', event.message);
					break;
			}
		});

		pollId = setInterval(async () => {
			if (!mounted) return;
			try {
				const state = await invoke<PlaybackState>('get_state');
				if (!mounted) return;
				playing = state === 'playing' || state === 'crossfading';
				const progress = await invoke<PlaybackProgressInfo | null>('get_progress');
				if (!mounted) return;
				if (progress) {
					if (!isSeeking) currentTime = progress.position_secs;
					if (progress.duration_secs) duration = progress.duration_secs;
				}
			} catch {}
		}, 500);

		invoke<PlaybackState>('get_state').then((s) => {
			if (!mounted) return;
			playing = s === 'playing' || s === 'crossfading';
			// If the engine is already playing on mount (e.g. the user
			// navigated away and back), fetch the live song info immediately —
			// a new `state_changed: playing` event will NOT fire.
			if (playing) refreshCurrentTrack();
		}).catch(() => {});

		return () => {
			mounted = false;
			if (pollId != null) clearInterval(pollId);
			if (unlistenPromise) unlistenPromise.then((un) => un());
		};
	});

	// ---------------------------------------------------------------------------
	// Restore last-played track on mount
	// ---------------------------------------------------------------------------

	$effect(() => {
		invoke<Record<string, unknown>>('get_settings').then(async (s) => {
			const lpt = s?.last_played_track as LastPlayedTrack | undefined;
			if (!lpt?.path) return;
			try {
				// If the engine is already playing, the `state_changed` event
				// will populate currentTrack from the live song — don't override
				// it with the stale persisted snapshot.
				const state = await invoke<PlaybackState>('get_state');
				if (state === 'playing' || state === 'crossfading') return;

				const exists = await invoke<boolean>('file_exists', { path: lpt.path });
				if (exists) {
					// Re-discover album art if the persisted path is missing
					// or points to a file that no longer exists.
					let artPath = lpt.album_art_path;
					if (artPath) {
						const artExists = await invoke<boolean>('file_exists', { path: artPath }).catch(() => false);
						if (!artExists) artPath = undefined;
					}
					if (!artPath) {
						try {
							artPath = await invoke<string | null>('get_album_art_path', { audioPath: lpt.path }) ?? undefined;
						} catch {}
					}
					currentTrack = {
						path: lpt.path,
						title: lpt.title,
						artist: lpt.artist,
						album: lpt.album,
						duration_secs: lpt.duration_secs,
						album_art_path: artPath,
					};
					loadAlbumArt(artPath);
					loadLyrics(lpt.path);
				}
			} catch {}
		}).catch(() => {});
	});

	// ---------------------------------------------------------------------------
	// Theme from album art → FluidBackground
	// ---------------------------------------------------------------------------

	$effect(() => {
		const url = albumArtBlobUrl;
		if (!url) {
			// Default theme: pale green/yellow palette matching the app identity.
			theme = {
				color: '#a3e635',
				palette: ['#a3e635', '#bef264', '#fde047', '#84cc16', '#65a30d'],
				light: false,
			};
			return;
		}
		let cancelled = false;
		console.log('[NowPlaying] extractTheme: url=', url.substring(0, 50));
		extractTheme(url).then((t) => {
			if (cancelled) return;
			console.log('[NowPlaying] extractTheme result:', t ? `palette=${t.palette.length} color=${t.color}` : 'null');
			if (t && t.palette.length > 0) {
				// Brighten the palette so the fluid background is always
				// vivid, even when the album art is predominantly dark.
				t.palette = t.palette.map(brightenColor);
				t.color = brightenColor(t.color);
			}
			theme = t ?? undefined;
		}).catch((e) => console.warn('[NowPlaying] extractTheme failed:', e));
		return () => { cancelled = true; };
	});

	/** Boost a hex color's luminance and saturation so it reads well as a
	 *  fluid-background gradient stop. Dark album art (e.g. classical covers)
	 *  would otherwise produce a near-black background. */
	function brightenColor(hex: string): string {
		let h = hex.replace('#', '');
		if (h.length === 3) h = h.split('').map((c) => c + c).join('');
		const r = parseInt(h.slice(0, 2), 16);
		const g = parseInt(h.slice(2, 4), 16);
		const b = parseInt(h.slice(4, 6), 16);
		const { hue, sat, lig } = rgbToHsl(r, g, b);
		// Lift luminance into a visible range [0.3, 0.7]. Blacks become
		// mid-dark, dark colours become mid, mids stay, lights stay light.
		let newL: number;
		if (lig < 0.3) {
			newL = 0.3 + lig * 0.3; // dark → 0.3..0.39
		} else if (lig > 0.85) {
			newL = lig; // keep whites/light colours as-is
		} else {
			newL = Math.min(0.85, lig * 1.15); // mid → slightly brighter
		}
		const newS = Math.max(0.3, sat);
		const { r: nr, g: ng, b: nb } = hslToRgb(hue, newS, newL);
		const toHex = (n: number) => Math.round(n).toString(16).padStart(2, '0');
		return `#${toHex(nr)}${toHex(ng)}${toHex(nb)}`;
	}

	function rgbToHsl(r: number, g: number, b: number): { hue: number; sat: number; lig: number } {
		r /= 255; g /= 255; b /= 255;
		const max = Math.max(r, g, b), min = Math.min(r, g, b);
		const lig = (max + min) / 2;
		let hue = 0;
		const sat = max === min ? 0 : lig > 0.5 ? (max - min) / (2 - max - min) : (max - min) / (max + min);
		if (max !== min) {
			const d = max - min;
			switch (max) {
				case r: hue = (g - b) / d + (g < b ? 6 : 0); break;
				case g: hue = (b - r) / d + 2; break;
				default: hue = (r - g) / d + 4;
			}
			hue /= 6;
		}
		return { hue: hue * 360, sat, lig };
	}

	function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
		h /= 360;
		const hue2rgb = (p: number, q: number, t: number) => {
			if (t < 0) t += 1; if (t > 1) t -= 1;
			if (t < 1/6) return p + (q - p) * 6 * t;
			if (t < 1/2) return q;
			if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
			return p;
		};
		let r, g, b;
		if (s === 0) { r = g = b = l; }
		else {
			const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
			const p = 2 * l - q;
			r = hue2rgb(p, q, h + 1/3);
			g = hue2rgb(p, q, h);
			b = hue2rgb(p, q, h - 1/3);
		}
		return { r: r * 255, g: g * 255, b: b * 255 };
	}

	const lyricTheme = $derived<LyricTheme | undefined>(
		theme ? { ...theme, light: true } : { color: '#111111', palette: [], light: true },
	);

	// ---------------------------------------------------------------------------
	// Transport controls
	// ---------------------------------------------------------------------------

	async function togglePlay() {
		if (playing) {
			try { await invoke('pause'); } catch {}
			return;
		}
		// Only resume if the engine is actually paused (song loaded + paused).
		// On app restart the engine is idle with no song — resume silently
		// does nothing, so we must call play() instead.
		try {
			const state = await invoke<PlaybackState>('get_state');
			if (state === 'paused') {
				await invoke('resume');
				return;
			}
		} catch {}
		// Engine is idle/stopped — start playing the current track.
		if (!currentTrack?.path) return;
		// Sync the queue index to the current track's position so
		// skip-next/prev advance from the right place (the track may
		// have been restored from `last_played_track` on app restart,
		// not via `playQueueIndex`).
		syncQueueIndexByPath(currentTrack.path);
		try {
			const probe = await invoke<{ sample_rate: number; channels: number; duration_secs: number; bits_per_sample?: number } | null>('probe_audio_file', { path: currentTrack.path }).catch(() => null);
			await invoke('play', {
				song: {
					path: currentTrack.path,
					title: currentTrack.title,
					artist: currentTrack.artist,
					album: currentTrack.album,
					album_art_path: currentTrack.album_art_path,
					sample_rate: probe?.sample_rate,
					bits_per_sample: probe?.bits_per_sample,
					duration_secs: probe?.duration_secs ?? currentTrack.duration_secs,
				},
			});
		} catch (e) {
			console.warn('[NowPlaying] play fallback failed:', e);
		}
	}

	async function skipNext() {
		if (playback.queue.length > 0) {
			try { await goNext(); } catch (e) { console.warn('[NowPlaying] goNext failed:', e); }
			return;
		}
		try { await invoke('skip_to_next'); } catch {}
	}

	async function skipPrev() {
		if (playback.queue.length > 0) {
			try { await goPrev(); } catch (e) { console.warn('[NowPlaying] goPrev failed:', e); }
			return;
		}
		try { await invoke('stop'); } catch {}
	}

	function onSeekInput(e: Event) {
		isSeeking = true;
		currentTime = Number((e.target as HTMLInputElement).value);
	}

	async function onSeekCommit(e: Event) {
		const position = Number((e.target as HTMLInputElement).value);
		isSeeking = false;
		try { await invoke('seek', { positionSecs: position }); } catch {}
	}

	const currentTimeMs = $derived(Math.round(currentTime * 1000));
	const seekMax = $derived(Math.max(0, duration ?? 0));
	const titleText = $derived(currentTrack?.title ?? 'Nothing playing');
	const artistText = $derived(currentTrack?.artist ?? '—');
	const albumText = $derived(currentTrack?.album ?? '');
	const timeLabel = $derived(`${fmtTime(currentTime)} / ${fmtTime(duration ?? 0)}`);

	const repeatIcon = $derived(
		playback.repeatMode === 'repeat-one' ? 'repeat-one'
		: playback.repeatMode === 'shuffle' ? 'shuffle'
		: 'repeat',
	);
	const repeatLabel = $derived(
		playback.repeatMode === 'sequential' ? 'Sequential'
		: playback.repeatMode === 'repeat-one' ? 'Loop song'
		: 'Shuffle',
	);

	function fmtTime(s: number): string {
		if (!Number.isFinite(s) || s < 0) s = 0;
		const m = Math.floor(s / 60);
		const sec = Math.floor(s % 60);
		return `${m}:${sec.toString().padStart(2, '0')}`;
	}
</script>

<section class="now-playing">
	<FluidBackground theme={theme} playing={playing} mode="fluid" album={albumArtBlobUrl} lowFreqVolume={0} brightnessMask={0.6} />

	<div class="layout">
		<!-- Left pane (40%): album art + info + controls -->
		<aside class="left-pane">
			<div class="art-wrapper">
				<div class="art-glass-backing"></div>
				<div class="art-slot">
					{#if albumArtBlobUrl}
						<img class="art-cover" src={albumArtBlobUrl} alt="" />
					{:else}
						<div class="art-placeholder">
							<Icon name="music" size={64} />
						</div>
					{/if}
				</div>
			</div>

			<div class="info-block">
				<div class="info-title">{titleText}</div>
				<div class="info-sub">
					<span class="info-artist">{artistText}</span>
					{#if albumText}
						<span class="info-sep" aria-hidden="true">·</span>
						<span class="info-album">{albumText}</span>
					{/if}
				</div>
			</div>

			<div class="seek-row">
				<span class="seek-time">{timeLabel}</span>
				<input type="range" min="0" max={seekMax} step="0.1"
					value={Math.min(currentTime, seekMax)}
					oninput={onSeekInput} onchange={onSeekCommit} aria-label="Seek" />
			</div>

			<div class="ctrl-row">
				<button
					type="button"
					class="ctrl-side"
					class:active={playback.repeatMode !== 'sequential'}
					onclick={cycleRepeatMode}
					aria-label={`Repeat mode: ${repeatLabel}`}
					title={`Repeat mode: ${repeatLabel}`}
				>
					<Icon name={repeatIcon as 'repeat' | 'repeat-one' | 'shuffle'} size={18} />
				</button>
				<button type="button" class="ctrl-skip" onclick={skipPrev} aria-label="Previous">
					<Icon name="skip-prev" size={16} />
				</button>
				<button type="button" class="ctrl-play" onclick={togglePlay} aria-label={playing ? 'Pause' : 'Play'}>
					{#if playing}
						<Icon name="pause" size={24} />
					{:else}
						<Icon name="play" size={24} />
					{/if}
				</button>
				<button type="button" class="ctrl-skip" onclick={skipNext} aria-label="Next">
					<Icon name="skip-next" size={16} />
				</button>
				<button
					type="button"
					class="ctrl-side"
					class:active={showQueue}
					onclick={() => (showQueue = !showQueue)}
					aria-label="Playback queue"
					title="Playback queue"
				>
					<Icon name="queue-list" size={18} />
				</button>
			</div>
		</aside>

		<!-- Right pane (60%): lyrics -->
		<main class="right-pane">
			{#if lyricLines.length > 0}
				<LyricPlayer
					lyrics={lyricLines}
					currentTime={currentTimeMs}
					{playing}
					onLineChange={() => {}}
					theme={lyricTheme}
					enableFluidBackground={false}
					enableBlur={true}
					enableScale={true}
					alignPosition={0.35}
					fontSize={appState.lyricFontSize}
				/>
			{:else if currentTrack}
				<div class="lyric-placeholder">
					<div class="lp-title">{currentTrack.title ?? currentTrack.path}</div>
					<div class="lp-sub">No lyrics found</div>
				</div>
			{:else}
				<div class="lyric-placeholder">
					<div class="lp-icon"><Icon name="music" size={36} /></div>
					<div class="lp-title">Scan your collection and select one to play</div>
				</div>
			{/if}
		</main>
	</div>

	<!-- Queue viewer overlay -->
	{#if showQueue}
		<div class="queue-overlay" role="dialog" aria-label="Playback queue">
			<div class="queue-panel">
				<header class="queue-header">
					<span class="queue-title">
						<Icon name="queue-list" size={16} />
						<span>Playback queue</span>
						<span class="queue-count">{playback.queue.length}</span>
					</span>
					<button type="button" class="queue-close" onclick={() => (showQueue = false)} aria-label="Close queue">
						<Icon name="close" size={16} />
					</button>
				</header>
				<div class="queue-list">
					{#if playback.queue.length === 0}
						<div class="queue-empty">
							<div class="queue-empty-icon"><Icon name="playlist" size={28} /></div>
							<p>Queue is empty. Play a song from the Library or Playlist to populate it.</p>
						</div>
					{:else}
						{#each playback.queue as track, i (track.path + i)}
							<button
								type="button"
								class="queue-row"
								class:active={i === playback.queueIndex}
								onclick={() => {
									void playQueueIndex(i);
								}}
							>
								<span class="queue-row-idx">
									{#if i === playback.queueIndex && playing}
										<Icon name="play" size={12} />
									{:else}
										{i + 1}
									{/if}
								</span>
								<span class="queue-row-body">
									<span class="queue-row-title">{track.title ?? track.path.split('/').pop() ?? track.path}</span>
									{#if track.artist}
										<span class="queue-row-artist">{track.artist}</span>
									{/if}
								</span>
							</button>
						{/each}
					{/if}
				</div>
			</div>
		</div>
	{/if}
</section>

<style>
	.now-playing {
		position: relative;
		height: 100%;
		width: 100%;
		overflow: hidden;
		font-family: 'Source Han Sans', sans-serif;
		background: radial-gradient(circle at 20% -10%, var(--uto-ambient-tint), transparent 55%), var(--uto-bg);
	}

	.layout {
		position: absolute;
		inset: 0;
		z-index: 1;
		display: flex;
		flex-direction: row;
		gap: 0;
	}

	/* ── Left pane (40%) ────────────────────────────────────────────────── */
	.left-pane {
		flex: 4;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px 24px;
		gap: 16px;
	}

	.art-wrapper {
		position: relative;
		display: inline-block;
		line-height: 0;
	}
	.art-glass-backing {
		position: absolute;
		top: -10px;
		left: -10px;
		right: -10px;
		bottom: -10px;
		border-radius: 20px;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		border: 1px solid var(--uto-glass-border);
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			0 8px 32px rgba(0, 0, 0, 0.25);
		z-index: 0;
	}
	.art-slot {
		position: relative;
		z-index: 1;
		width: min(40vh, 300px);
		aspect-ratio: 1;
		border-radius: 12px;
		overflow: hidden;
		box-shadow: 0 12px 48px rgba(0, 0, 0, 0.4);
	}
	.art-cover {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.art-placeholder {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(190, 242, 100, 0.06);
		color: var(--uto-text-faint);
	}

	.info-block {
		text-align: center;
		max-width: 320px;
		overflow: hidden;
	}
	.info-title {
		font-size: 18px;
		font-weight: 600;
		color: var(--uto-text-strong);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.info-sub {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		font-size: 13px;
		color: var(--uto-text);
		margin-top: 4px;
	}
	.info-artist { white-space: nowrap; }
	.info-album { color: var(--uto-text-muted); white-space: nowrap; }
	.info-sep { color: var(--uto-text-faint); }

	.seek-row {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		max-width: 300px;
	}
	.seek-time {
		font-variant-numeric: tabular-nums;
		font-size: 12px;
		color: var(--uto-text);
		min-width: 88px;
		text-align: right;
	}
	.seek-row input[type='range'] {
		flex: 1;
		accent-color: var(--uto-accent-green, #bef264);
		height: 4px;
	}

	.ctrl-row {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 20px;
	}
	.ctrl-skip {
		width: 40px; height: 40px; border-radius: 50%;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		color: var(--uto-text);
		cursor: pointer;
		display: flex; align-items: center; justify-content: center;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1), transform 0.18s;
	}
	.ctrl-skip:hover { background: rgba(190,242,100,0.12); color: var(--uto-text-strong); transform: scale(1.06); }
	.ctrl-skip:active { transform: scale(0.94); }
	.ctrl-play {
		width: 56px; height: 56px; border-radius: 50%;
		border: none;
		background: var(--uto-accent-green, #bef264);
		color: var(--uto-play-text);
		cursor: pointer;
		display: flex; align-items: center; justify-content: center;
		transition: transform 0.12s, filter 0.12s;
		box-shadow: 0 8px 24px rgba(190,242,100,0.25);
	}
	.ctrl-play:hover { transform: scale(1.05); filter: brightness(1.06); }
	.ctrl-play:active { transform: scale(0.96); }

	.ctrl-side {
		width: 36px; height: 36px; border-radius: 50%;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		color: var(--uto-text-muted);
		cursor: pointer;
		display: flex; align-items: center; justify-content: center;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1), color 0.18s, transform 0.18s;
	}
	.ctrl-side:hover { background: rgba(190,242,100,0.12); color: var(--uto-text-strong); transform: scale(1.06); }
	.ctrl-side:active { transform: scale(0.94); }
	.ctrl-side.active {
		color: var(--uto-accent-green, #bef264);
		border-color: rgba(190, 242, 100, 0.3);
		background: rgba(190, 242, 100, 0.1);
	}

	/* ── Right pane (60%) ────────────────────────────────────────────────── */
	.right-pane {
		flex: 6;
		position: relative;
		overflow: hidden;
	}

	.lyric-placeholder {
		display: flex; flex-direction: column;
		align-items: center; justify-content: center;
		height: 100%; gap: 10px;
		color: var(--uto-text-faint);
		padding: 0 24px;
	}
	.lp-icon { opacity: 0.3; }
	.lp-title {
		font-size: 18px; font-weight: 500; color: var(--uto-text);
		text-align: center; max-width: 300px;
	}
	.lp-sub { font-size: 13px; color: var(--uto-text-muted); }

	/* ── Queue viewer overlay ─────────────────────────────────────────────── */
	.queue-overlay {
		position: absolute;
		inset: 0;
		z-index: 10;
		background: rgba(0, 0, 0, 0.35);
		backdrop-filter: blur(6px);
		-webkit-backdrop-filter: blur(6px);
		display: flex;
		justify-content: flex-end;
		animation: queue-fade 0.18s ease-out;
	}
	@keyframes queue-fade {
		from { opacity: 0; }
		to { opacity: 1; }
	}
	.queue-panel {
		width: min(380px, 88vw);
		height: 100%;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			-12px 0 48px rgba(0, 0, 0, 0.4);
		border-left: 1px solid var(--uto-glass-border);
		display: flex;
		flex-direction: column;
		animation: queue-slide 0.22s cubic-bezier(0.22, 1, 0.36, 1);
	}
	@keyframes queue-slide {
		from { transform: translateX(24px); opacity: 0.6; }
		to { transform: translateX(0); opacity: 1; }
	}
	.queue-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px 16px;
		border-bottom: 1px solid var(--uto-glass-border);
	}
	.queue-title {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 14px;
		font-weight: 600;
		color: var(--uto-text-strong);
	}
	.queue-count {
		font-size: 11px;
		font-weight: 500;
		color: var(--uto-text-muted);
		background: rgba(190, 242, 100, 0.12);
		padding: 1px 8px;
		border-radius: 999px;
	}
	.queue-close {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		cursor: pointer;
		width: 28px;
		height: 28px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.18s, color 0.18s;
	}
	.queue-close:hover {
		background: rgba(252, 165, 165, 0.1);
		color: #fca5a5;
	}
	.queue-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.queue-list::-webkit-scrollbar { width: 6px; }
	.queue-list::-webkit-scrollbar-thumb { background: var(--uto-scrollbar-thumb); border-radius: 6px; }
	.queue-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		height: 100%;
		text-align: center;
		padding: 24px;
		color: var(--uto-text-faint);
		font-size: 13px;
	}
	.queue-empty-icon { opacity: 0.4; }
	.queue-row {
		appearance: none;
		width: 100%;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 10px;
		border: none;
		border-radius: 10px;
		background: transparent;
		color: var(--uto-text);
		font-family: inherit;
		text-align: left;
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.queue-row:hover { background: rgba(190, 242, 100, 0.08); }
	.queue-row.active {
		background: rgba(190, 242, 100, 0.14);
	}
	.queue-row.active .queue-row-title { color: var(--uto-accent-green, #bef264); }
	.queue-row-idx {
		flex-shrink: 0;
		width: 24px;
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		color: var(--uto-text-faint);
		text-align: center;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.queue-row.active .queue-row-idx { color: var(--uto-accent-green, #bef264); }
	.queue-row-body {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		flex: 1;
	}
	.queue-row-title {
		font-size: 13px;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.queue-row-artist {
		font-size: 11px;
		color: var(--uto-text-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* ── Responsive: stack on mobile ─────────────────────────────────────── */
	@media (max-width: 768px) {
		.layout { flex-direction: column; }
		.left-pane {
			flex: none;
			padding: 20px 16px 12px;
			gap: 10px;
		}
		.art-slot {
			width: 160px;
		}
		.right-pane {
			flex: 1;
			min-height: 0;
		}
		.seek-row { max-width: 240px; }
	}
</style>
