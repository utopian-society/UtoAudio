<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { parseM3u8, stringifyM3u8 } from '../lib/m3u8';
	import type { M3u8Track } from '../lib/m3u8';
	import Icon from '../components/Icon.svelte';
	import { LiquidGlass } from '../lib/liquid-glass';

	// -----------------------------------------------------------------------
	// Types mirroring the audio backend's serde shapes (camelCase per Tauri).
	// -----------------------------------------------------------------------

	/** Mirrors `audio_core::tauri_api::SongInfo`. Subset needed for `play`. */
	interface SongInfo {
		path: string;
		title?: string;
		artist?: string;
		album?: string;
		duration_secs?: number;
	}

	// -----------------------------------------------------------------------
	// Reactive page state (Svelte 5 runes)
	// -----------------------------------------------------------------------

	let tracks = $state<M3u8Track[]>([]);
	let currentIndex = $state(-1);
	let playlistName = $state('New Playlist');
	let playlistPath = $state('');
	/** Whether the playlist has unsaved edits (used to drive the "Save" hint). */
	let dirty = $state(false);

	/** The currently-playing track path (from the engine), for highlight. */
	let playingPath = $state('');

	// -----------------------------------------------------------------------
	// Engine state sync — pull `current_path` once on mount + on a periodic
	// tick so the active row picks up changes triggered from Now Playing.
	// -----------------------------------------------------------------------

	$effect(() => {
		let mounted = true;
		let pollId: ReturnType<typeof setInterval> | null = null;

		async function refresh(): Promise<void> {
			try {
				const p = await invoke<string | null>('current_path');
				if (mounted && p !== null) playingPath = p;
			} catch {
				/* engine may not be ready yet — ignore */
			}
		}

		refresh();
		pollId = setInterval(refresh, 1500);

		return () => {
			mounted = false;
			if (pollId != null) clearInterval(pollId);
		};
	});

	// -----------------------------------------------------------------------
	// Derived display values
	// -----------------------------------------------------------------------

	const trackCount = $derived(tracks.length);
	const totalDuration = $derived(
		tracks.reduce((acc, t) => acc + (t.duration ?? 0), 0),
	);
	const totalLabel = $derived(formatDuration(totalDuration, true));

	function formatDuration(secs: number, allowEmpty = false): string {
		if (!Number.isFinite(secs) || secs <= 0) return allowEmpty ? '0:00' : '—';
		const m = Math.floor(secs / 60);
		const s = Math.floor(secs % 60);
		return `${m}:${s.toString().padStart(2, '0')}`;
	}

	// -----------------------------------------------------------------------
	// Playlist operations
	// -----------------------------------------------------------------------

	function newPlaylist(): void {
		tracks = [];
		currentIndex = -1;
		playlistName = 'New Playlist';
		playlistPath = '';
		dirty = false;
	}

	function clearPlaylist(): void {
		tracks = [];
		currentIndex = -1;
		dirty = true;
	}

	/** Open a `.m3u8` file and populate the track list. */
	async function openPlaylist(): Promise<void> {
		// The Tauri dialog plugin isn't installed (see AGENTS.md / prompt-5),
		// so we use a hidden `<input type="file">` driven by a ref. The actual
		// file read is done via the browser's File API — no `fs` plugin needed.
		const file = await pickSingleFile('.m3u8');
		if (!file) return;
		const content = await file.text();
		const baseDir = file.name.includes('/') ? file.name : '';
		const parsed = parseM3u8(content, baseDir || undefined);
		tracks = parsed;
		playlistName = file.name.replace(/\.m3u8?$/i, '') || 'Playlist';
		playlistPath = file.name;
		currentIndex = -1;
		dirty = false;
	}

	/** Save the playlist back to its current path (or Save As if none). */
	async function savePlaylist(): Promise<void> {
		if (!playlistPath) {
			await savePlaylistAs();
			return;
		}
		await writePlaylist(playlistPath);
		dirty = false;
	}

	/** Prompt the user for a new path and save there. */
	async function savePlaylistAs(): Promise<void> {
		await writePlaylist(`${playlistName || 'playlist'}.m3u8`);
		dirty = false;
	}

	async function writePlaylist(path: string): Promise<void> {
		// Browser-only environment: we can't actually write to disk without the
		// `@tauri-apps/plugin-fs` plugin. For the MVP we fall back to a
		// download-style save when in a browser context. Tauri's `dialog.save`
		// is not available either. This stub keeps the API surface intact — a
		// later prompt wires `invoke('write_playlist', { path, content })`.
		const content = stringifyM3u8(tracks, playlistName);
		const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
		const a = document.createElement('a');
		a.href = URL.createObjectURL(blob);
		a.download = path.split('/').pop() || 'playlist.m3u8';
		a.click();
		URL.revokeObjectURL(a.href);
		playlistPath = path;
	}

	/** Add audio files picked from the OS file dialog. */
	async function addFiles(): Promise<void> {
		const files = await pickMultipleFiles(
			'.flac,.wav,.mp3,.opus,.ogg,.aac,.m4a,.wv,.dsf,.dff,.aiff,.ape,.wma',
		);
		if (!files) return;
		for (const file of files) {
			tracks.push({
				path: file.name,
				title: file.name.replace(/\.[^/.]+$/, ''),
			});
		}
		dirty = true;
	}

	function playTrack(index: number): void {
		const track = tracks[index];
		if (!track) return;
		currentIndex = index;
		const song: SongInfo = {
			path: track.path,
			title: track.title,
			artist: track.artist,
			duration_secs: track.duration,
		};
		invoke('play', { song }).catch((e) =>
			console.error('[Playlist] play failed:', e),
		);
	}

	function queueNext(index: number): void {
		const track = tracks[index];
		if (!track) return;
		const song: SongInfo = {
			path: track.path,
			title: track.title,
			artist: track.artist,
			duration_secs: track.duration,
		};
		invoke('queue_next', { song }).catch((e) =>
			console.error('[Playlist] queue_next failed:', e),
		);
	}

	function removeTrack(index: number): void {
		tracks.splice(index, 1);
		if (currentIndex === index) currentIndex = -1;
		else if (currentIndex > index) currentIndex -= 1;
		dirty = true;
	}

	function moveTrackUp(index: number): void {
		if (index === 0) return;
		const [track] = tracks.splice(index, 1);
		tracks.splice(index - 1, 0, track);
		if (currentIndex === index) currentIndex -= 1;
		else if (currentIndex === index - 1) currentIndex += 1;
		dirty = true;
	}

	function moveTrackDown(index: number): void {
		if (index === tracks.length - 1) return;
		const [track] = tracks.splice(index, 1);
		tracks.splice(index + 1, 0, track);
		if (currentIndex === index) currentIndex += 1;
		else if (currentIndex === index + 1) currentIndex -= 1;
		dirty = true;
	}

	// -----------------------------------------------------------------------
	// File picker helpers
	// -----------------------------------------------------------------------

	/** Open a hidden single-selection file input, resolve to the picked file or null. */
	function pickSingleFile(accept: string): Promise<File | null> {
		return new Promise((resolve) => {
			const input = document.createElement('input');
			input.type = 'file';
			input.accept = accept;
			input.multiple = false;
			let resolved = false;
			input.onchange = () => {
				if (resolved) return;
				resolved = true;
				const files = Array.from(input.files ?? []);
				resolve(files[0] ?? null);
			};
			// Cancel has no reliable browser hook — the promise hangs until
			// a successful selection (consistent with `<input type="file">`).
			input.click();
		});
	}

	/** Open a hidden multiple-selection file input, resolve to the picked files or null. */
	function pickMultipleFiles(accept: string): Promise<File[] | null> {
		return new Promise((resolve) => {
			const input = document.createElement('input');
			input.type = 'file';
			input.accept = accept;
			input.multiple = true;
			let resolved = false;
			input.onchange = () => {
				if (resolved) return;
				resolved = true;
				const files = Array.from(input.files ?? []);
				resolve(files.length > 0 ? files : null);
			};
			input.click();
		});
	}

	// -----------------------------------------------------------------------
	// Template helpers
	// -----------------------------------------------------------------------

	function trackLabel(t: M3u8Track): string {
		if (t.title && t.artist) return `${t.artist} - ${t.title}`;
		return t.title || t.path.split('/').pop() || t.path;
	}

	function isPlaying(index: number): boolean {
		return tracks[index]?.path === playingPath;
	}
</script>

<section class="page">
	<LiquidGlass roundness={0} accent="#bef264" contrast="light">
		<header class="header">
		<div class="title-wrap">
			<input
				class="name-input"
				type="text"
				bind:value={playlistName}
				placeholder="Playlist name"
				aria-label="Playlist name"
				oninput={() => (dirty = true)}
			/>
			<span class="meta">
				<span>{trackCount} track{trackCount === 1 ? '' : 's'}</span>
				<span class="dot">·</span>
				<span>{totalLabel}</span>
				{#if dirty}<span class="dirty">● unsaved</span>{/if}
			</span>
		</div>
		<div class="actions">
			<button class="btn" type="button" onclick={newPlaylist}>New</button>
			<button class="btn" type="button" onclick={openPlaylist}>Open</button>
			<button class="btn" type="button" onclick={savePlaylist}>Save</button>
			<button class="btn" type="button" onclick={savePlaylistAs}>Save As</button>
			<button class="btn danger" type="button" onclick={clearPlaylist}>Clear</button>
		</div>
	</header>
	</LiquidGlass>

	<div class="list" role="list">
		{#if tracks.length === 0}
			<div class="empty">
				<div class="empty-icon"><Icon name="playlist" size={32} /></div>
				<p class="empty-title">No tracks yet</p>
				<p class="empty-sub">Open a playlist or add some files.</p>
			</div>
		{:else}
			{#each tracks as track, i (track.path + i)}
				<div
					class="track-row"
					class:active={currentIndex === i}
					class:playing={isPlaying(i)}
					role="listitem"
				>
					<span class="index">{i + 1}</span>
					<button
						class="row-main"
						type="button"
						onclick={() => playTrack(i)}
						ondblclick={() => queueNext(i)}
						aria-label={`Play ${trackLabel(track)}`}
					>
						<span class="row-title">{trackLabel(track)}</span>
						<span class="row-sub"
							>{track.artist ?? 'Unknown artist'} · {track.title ?? track.path.split('/').pop()}</span>
					</button>
					<span class="duration">{formatDuration(track.duration ?? 0)}</span>
					<div class="row-actions">
						<button type="button" class="icon-btn" onclick={() => queueNext(i)} aria-label="Queue next"><Icon name="skip-next" size={14} /></button>
						<button type="button" class="icon-btn" onclick={() => moveTrackUp(i)} aria-label="Move up"><Icon name="arrow-up" size={14} /></button>
						<button type="button" class="icon-btn" onclick={() => moveTrackDown(i)} aria-label="Move down"><Icon name="arrow-down" size={14} /></button>
						<button type="button" class="icon-btn danger" onclick={() => removeTrack(i)} aria-label="Remove"><Icon name="close" size={14} /></button>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<LiquidGlass roundness={0} accent="#bef264" contrast="light">
		<footer class="footer">
		<button class="btn primary" type="button" onclick={addFiles}>Add files…</button>
		{#if playlistPath}
			<span class="path">{playlistPath}</span>
		{/if}
	</footer>
	</LiquidGlass>
</section>

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
		width: 100%;
		background: radial-gradient(circle at 20% -10%, var(--uto-ambient-tint), transparent 55%), var(--uto-bg);
		color: var(--uto-text);
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		overflow: hidden;
	}

	/* --- Header -------------------------------------------------------------- */
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 16px 20px 14px;
	}
	.title-wrap {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
		flex: 1;
	}
	.name-input {
		appearance: none;
		background: transparent;
		border: none;
		outline: none;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: -0.01em;
		color: var(--uto-text-strong);
		font-family: inherit;
		padding: 2px 4px;
		border-radius: 6px;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.name-input:hover,
	.name-input:focus {
		background: var(--uto-surface);
	}
	.meta {
		display: flex;
		gap: 6px;
		align-items: center;
		font-size: 12px;
		color: var(--uto-text-muted);
		letter-spacing: 0.02em;
		padding-left: 4px;
	}
	.meta .dot {
		opacity: 0.5;
	}
	.meta .dirty {
		color: var(--uto-accent-yellow, #fef08a);
		font-weight: 500;
	}

	.actions {
		display: flex;
		gap: 6px;
		flex-shrink: 0;
	}
	.btn {
		appearance: none;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 4px 16px var(--uto-glass-outer-shadow);
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		font-weight: 500;
		padding: 7px 12px;
		border-radius: 10px;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1),
			border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			transform 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.btn:hover {
		background: rgba(190, 242, 100, 0.08);
		color: var(--uto-text-strong);
		border-color: rgba(190, 242, 100, 0.15);
		transform: translateY(-1px);
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 12px 36px var(--uto-glass-outer-shadow);
	}
	.btn:active {
		transform: scale(0.97);
	}
	.btn.primary {
		background: rgba(190, 242, 100, 0.14);
		color: var(--uto-text-strong);
		border-color: rgba(190, 242, 100, 0.28);
	}
	.btn.primary:hover {
		background: rgba(190, 242, 100, 0.22);
	}
	.btn.danger:hover {
		color: #fca5a5;
		border-color: rgba(252, 165, 165, 0.3);
		background: rgba(252, 165, 165, 0.06);
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* --- Track list ---------------------------------------------------------- */
	.list {
		flex: 1;
		overflow-y: auto;
		overflow-x: hidden;
		padding: 8px 12px 8px 12px;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.list::-webkit-scrollbar {
		width: 8px;
	}
	.list::-webkit-scrollbar-thumb {
		background: var(--uto-scrollbar-thumb);
		border-radius: 8px;
	}
	.list::-webkit-scrollbar-thumb:hover {
		background: var(--uto-scrollbar-thumb-hover);
	}

	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
		color: var(--uto-text-faint);
		padding: 64px 24px;
		text-align: center;
	}
	.empty-icon {
		font-size: 36px;
		opacity: 0.4;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.empty-title {
		font-size: 15px;
		color: var(--uto-text-muted);
		margin: 0;
		font-weight: 500;
	}
	.empty-sub {
		font-size: 13px;
		margin: 0;
	}

	.track-row {
		display: grid;
		grid-template-columns: 32px 1fr auto auto;
		align-items: center;
		gap: 12px;
		padding: 8px 12px;
		margin: 2px 0;
		border-radius: 12px;
		border: 1px solid var(--uto-glass-border);
		border-left: 2px solid transparent;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 4px 16px var(--uto-glass-outer-shadow);
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1),
			border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			transform 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.track-row:hover {
		background: rgba(190, 242, 100, 0.06);
		border-color: rgba(190, 242, 100, 0.15);
		transform: translateY(-1px);
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 8px 32px var(--uto-glass-outer-shadow);
	}
	.track-row.active {
		background: rgba(190, 242, 100, 0.08);
		border-left-color: var(--uto-accent-green, #bef264);
	}
	.track-row.playing {
		border-left-color: var(--uto-accent-yellow, #fef08a);
	}
	.track-row .index {
		font-variant-numeric: tabular-nums;
		font-size: 12px;
		color: var(--uto-text-faint);
		text-align: right;
		padding-right: 4px;
	}
	.track-row.active .index {
		color: var(--uto-accent-green, #bef264);
	}

	.row-main {
		appearance: none;
		background: transparent;
		border: none;
		cursor: pointer;
		text-align: left;
		display: flex;
		flex-direction: column;
		min-width: 0;
		gap: 2px;
		padding: 2px 0;
		font-family: inherit;
		color: inherit;
	}
	.row-title {
		font-size: 14px;
		font-weight: 500;
		color: var(--uto-text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.row-sub {
		font-size: 12px;
		color: var(--uto-text-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.duration {
		font-variant-numeric: tabular-nums;
		font-size: 12px;
		color: var(--uto-text-muted);
		min-width: 44px;
		text-align: right;
	}
	.row-actions {
		display: flex;
		gap: 2px;
		opacity: 0;
		transition: opacity 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.track-row:hover .row-actions,
	.track-row.active .row-actions {
		opacity: 1;
	}
	.icon-btn {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		font-size: 14px;
		width: 26px;
		height: 26px;
		border-radius: 7px;
		cursor: pointer;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.icon-btn:hover {
		background: rgba(190, 242, 100, 0.08);
		color: var(--uto-text);
	}
	.icon-btn.danger:hover {
		color: #fca5a5;
		background: rgba(252, 165, 165, 0.1);
	}

	/* --- Footer -------------------------------------------------------------- */
	.footer {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 12px 20px 14px;
	}
	.path {
		font-size: 12px;
		color: var(--uto-text-faint);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	/* --- Mobile -------------------------------------------------------------- */
	@media (max-width: 768px) {
		.header {
			flex-direction: column;
			align-items: stretch;
			gap: 10px;
		}
		.actions {
			overflow-x: auto;
			justify-content: flex-start;
		}
		.track-row {
			grid-template-columns: 24px 1fr auto;
		}
		.duration {
			display: none;
		}
		.row-actions {
			opacity: 1;
		}
	}
</style>
