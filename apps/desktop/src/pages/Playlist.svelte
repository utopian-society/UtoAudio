<script lang="ts">
	import { invoke, convertFileSrc } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';
	import { readTextFile } from '@tauri-apps/plugin-fs';
	import Icon from '../components/Icon.svelte';
	import { LiquidGlass } from '../lib/liquid-glass';
	import { setQueue, type QueueTrack } from '../lib/playback.svelte';
	import { AUDIO_EXTENSIONS } from '../lib/file-browser';

	// -----------------------------------------------------------------------
	// Types (mirrors Rust `audio_ffi::playlist` serde structs)
	// -----------------------------------------------------------------------

	interface PlaylistInfo {
		id: number;
		name: string;
		trackCount: number;
		createdAt: number;
		updatedAt: number;
	}

	interface PlaylistTrackRow {
		id: number;
		playlistId: number;
		position: number;
		path: string;
		title: string;
		artist: string;
		album: string;
		durationSecs: number;
		albumArtPath?: string;
		sampleRate?: number;
		bitsPerSample?: number;
		fileSize?: number;
	}

interface M3u8ImportResult {
	importedCount: number;
	missingPaths: string[];
}

	// -----------------------------------------------------------------------
	// Types
	// -----------------------------------------------------------------------

	interface SongInfo {
		path: string;
		title?: string;
		artist?: string;
		album?: string;
		duration_secs?: number;
		album_art_path?: string;
		sample_rate?: number;
		bits_per_sample?: number;
	}

	async function probeSong(path: string): Promise<{ sample_rate?: number; bits_per_sample?: number; duration_secs?: number }> {
		try {
			const info = await invoke<{ sample_rate: number; channels: number; duration_secs: number; bits_per_sample?: number }>('probe_audio_file', { path });
			return { sample_rate: info.sample_rate, bits_per_sample: info.bits_per_sample, duration_secs: info.duration_secs };
		} catch { return {}; }
	}

	// -----------------------------------------------------------------------
	// Playlist list state (sidebar)
	// -----------------------------------------------------------------------

	let playlists = $state<PlaylistInfo[]>([]);
	let selectedId = $state<number | null>(null);
	let selectedName = $state('');
	let dirty = $state(false);

	// -----------------------------------------------------------------------
	// Track list state (main area)
	// -----------------------------------------------------------------------

	let tracks = $state<PlaylistTrackRow[]>([]);
	let loading = $state(false);
	let importing = $state(false);
	let importError = $state('');
	let missingPaths = $state<string[]>([]);
	let lastError = $state('');

	// -----------------------------------------------------------------------
	// Playing path highlight
	// -----------------------------------------------------------------------

	let playingPath = $state('');

	$effect(() => {
		let mounted = true;
		let pollId: ReturnType<typeof setInterval> | null = null;

		async function refresh(): Promise<void> {
			try {
				const p = await invoke<string | null>('current_path');
				if (mounted && p !== null) playingPath = p;
			} catch { /* ignore */ }
		}

		refresh();
		pollId = setInterval(refresh, 1500);
		return () => { mounted = false; if (pollId != null) clearInterval(pollId); };
	});

// -----------------------------------------------------------------------
	// Album art — loaded in batches of 50 when a playlist is selected.
	// This prevents OOM kills on large playlists while still loading
	// everything (no need for scroll-based lazy loading).
	// -----------------------------------------------------------------------

	let artUrls: Record<string, string> = $state({});

	async function loadAlbumArts(): Promise<void> {
		const paths = tracks
			.map(t => t.albumArtPath)
			.filter((p): p is string => !!p);
		if (paths.length === 0) return;
		const TOTAL = paths.length;
		const BATCH_SIZE = 50;
		for (let i = 0; i < TOTAL; i += BATCH_SIZE) {
			const batch = paths.slice(i, i + BATCH_SIZE);
			await Promise.allSettled(batch.map(async (artPath) => {
				try { artUrls[artPath] = convertFileSrc(artPath); } catch {}
			}));
			console.debug(`Album art ${Math.min(i + BATCH_SIZE, TOTAL)}/${TOTAL}`);
			await new Promise(r => setTimeout(r, 0));
		}
	}

	function artSrc(albumArtPath: string | null | undefined): string | null {
		if (!albumArtPath) return null;
		return artUrls[albumArtPath] ?? null;
	}

	// -----------------------------------------------------------------------
	// Derived values
	// -----------------------------------------------------------------------

	const trackCount = $derived(tracks.length);
	const totalDuration = $derived(tracks.reduce((a, t) => a + (t.durationSecs ?? 0), 0));

	function fmtDuration(secs: number): string {
		if (!Number.isFinite(secs) || secs <= 0) return '—';
		const m = Math.floor(secs / 60);
		const s = Math.floor(secs % 60);
		return `${m}:${s.toString().padStart(2, '0')}`;
	}

	function formatInfo(t: PlaylistTrackRow): string {
		const parts: string[] = [];
		const ext = t.path.split('.').pop()?.toUpperCase() ?? '';
		if (ext) parts.push(ext);
		if (t.bitsPerSample) parts.push(`${t.bitsPerSample}bit`);
		if (t.sampleRate) {
			const khz = (t.sampleRate / 1000).toFixed(1).replace(/\.0$/, '');
			parts.push(`${khz}kHz`);
		}
		return parts.join(' · ');
	}

	// -----------------------------------------------------------------------
	// Playlist CRUD
	// -----------------------------------------------------------------------

	async function loadPlaylists(): Promise<void> {
		try {
			playlists = await invoke<PlaylistInfo[]>('list_playlists');
		} catch (e) { reportError(e); }
	}

	async function selectPlaylist(id: number | null): Promise<void> {
		selectedId = id;
		missingPaths = [];
		importError = '';
		if (id == null) {
			selectedName = '';
			tracks = [];
			return;
		}
		loading = true;
		try {
			const pl = playlists.find(p => p.id === id);
			selectedName = pl?.name ?? '';
			tracks = await invoke<PlaylistTrackRow[]>('get_playlist_tracks', { id });
			// Load album art in batches to avoid OOM on large playlists
			loadAlbumArts();
		} catch (e) { reportError(e); }
		finally { loading = false; }
	}

	let createMenuOpen = $state(false);

	async function createPlaylist(): Promise<void> {
		try {
			const id = await invoke<number>('create_playlist', { name: 'New Playlist' });
			await loadPlaylists();
			await selectPlaylist(id);
		} catch (e) { reportError(e); }
	}

	async function createAndImportM3u8(): Promise<void> {
		try {
			const picked = await open({
				multiple: false,
				filters: [{ name: 'Playlist', extensions: ['m3u8', 'm3u'] }],
			});
			if (typeof picked !== 'string') return;

			// Extract filename without extension for playlist name
			const lastSep = Math.max(picked.lastIndexOf('/'), picked.lastIndexOf('\\'));
			const filename = picked.substring(lastSep + 1);
			const nameWithoutExt = filename.replace(/\.[^/.]+$/, '');

			const id = await invoke<number>('create_playlist', { name: nameWithoutExt });
			await loadPlaylists();
			await selectPlaylist(id);

			// Import the m3u8 into the new playlist
			const content = await readTextFile(picked);
			await invoke<M3u8ImportResult>('import_m3u8_to_playlist_with_root', {
				playlistId: id,
				content,
				m3u8Path: picked,
			});
			// Refresh the track list
			const newTracks = await invoke<PlaylistTrackRow[]>('get_playlist_tracks', { id });
			tracks = newTracks;
		} catch (e) { reportError(e); }
	}

	async function importM3u8(): Promise<void> {
		try {
			const picked = await open({
				multiple: false,
				filters: [{ name: 'Playlist', extensions: ['m3u8', 'm3u'] }],
			});
			if (typeof picked !== 'string') return;

			// Extract filename without extension for playlist name
			const lastSep = Math.max(picked.lastIndexOf('/'), picked.lastIndexOf('\\'));
			const filename = picked.substring(lastSep + 1);
			const nameWithoutExt = filename.replace(/\.[^/.]+$/, '');

			const id = await invoke<number>('create_playlist', { name: nameWithoutExt });
			await loadPlaylists();
			await selectPlaylist(id);

			// Now import the m3u8 into the new playlist
			const content = await readTextFile(picked);
			await invoke<M3u8ImportResult>('import_m3u8_to_playlist_with_root', {
				playlistId: id,
				content,
				m3u8Path: picked,
			});
			// Refresh the track list
			const fresh = await invoke<PlaylistTrackRow[]>('get_playlist_tracks', { id });
			tracks = fresh;
		} catch (e) { reportError(e); }
	}

	async function renamePlaylist(id: number, name: string): Promise<void> {
		if (!name.trim()) return;
		try {
			await invoke('rename_playlist', { id, name: name.trim() });
			await loadPlaylists();
			if (selectedId === id) {
				selectedName = name.trim();
				const pl = playlists.find(p => p.id === id);
				if (pl) pl.name = selectedName;
			}
		} catch (e) { reportError(e); }
	}

	async function deletePlaylist(id: number): Promise<void> {
		if (!confirm('Delete this playlist?')) return;
		try {
			await invoke('delete_playlist', { id });
			if (selectedId === id) selectPlaylist(null);
			await loadPlaylists();
		} catch (e) { reportError(e); }
	}

	// -----------------------------------------------------------------------
	// m3u8 import
	// -----------------------------------------------------------------------

	let importDialogOpen = $state(false);
	let importFilePath = $state('');
	let importBaseDir = $state('');
	let importCoverMissing = $state(false);

	function openImportDialog(): void {
		importDialogOpen = true;
		importFilePath = '';
		importBaseDir = '';
		importCoverMissing = false;
		missingPaths = [];
		importError = '';
	}

	async function pickM3u8(): Promise<void> {
		try {
			const picked = await open({
				multiple: false,
				filters: [{ name: 'Playlist', extensions: ['m3u8', 'm3u'] }],
			});
			if (typeof picked === 'string') {
				importFilePath = picked;
				const lastSep = Math.max(picked.lastIndexOf('/'), picked.lastIndexOf('\\'));
				importBaseDir = lastSep >= 0 ? picked.substring(0, lastSep) : '';
			}
		} catch (e) {
			reportError(e);
		}
	}

	async function doImport(): Promise<void> {
		if (!importFilePath || selectedId == null) return;
		importing = true;
		importError = '';
		missingPaths = [];
		try {
			const content = await readTextFile(importFilePath);
			const result = await invoke<M3u8ImportResult>('import_m3u8_to_playlist_with_root', {
				playlistId: selectedId,
				content,
				m3u8Path: importFilePath,
			});
			// Fetch the updated track list from the DB
			tracks = await invoke<PlaylistTrackRow[]>('get_playlist_tracks', { id: selectedId });
			importCoverMissing = result.missingPaths.length > 0;
			missingPaths = result.missingPaths;
			dirty = false;
		} catch (e) {
			importError = String(e);
		} finally {
			importing = false;
			importDialogOpen = false;
		}
		await loadPlaylists();
	}

	// -----------------------------------------------------------------------
	// Add files
	// -----------------------------------------------------------------------

	async function addFiles(): Promise<void> {
		if (selectedId == null) return;
		const files = await pickMultipleFiles(AUDIO_EXTENSIONS.join(','));
		if (!files || files.length === 0) return;
		const paths: string[] = [];
		for (const f of files) {
			const path = (f as unknown as { path?: string }).path ?? f.name;
			paths.push(path);
		}
		loading = true;
		try {
			const result = await invoke<PlaylistTrackRow[]>('add_tracks_to_playlist', {
				playlistId: selectedId,
				paths,
			});
			tracks = [...tracks, ...result];
			await loadPlaylists();
		} catch (e) { reportError(e); }
		finally { loading = false; }
	}

	// -----------------------------------------------------------------------
	// Track operations
	// -----------------------------------------------------------------------

	async function removeTrack(id: number): Promise<void> {
		try {
			await invoke('remove_playlist_track', { trackId: id });
			tracks = tracks.filter(t => t.id !== id);
			await loadPlaylists();
		} catch (e) { reportError(e); }
	}

	async function moveTrack(trackId: number, direction: 'up' | 'down'): Promise<void> {
		try {
			await invoke('move_playlist_track', { trackId, direction });
			if (direction === 'up') {
				const idx = tracks.findIndex(t => t.id === trackId);
				if (idx > 0) [tracks[idx - 1], tracks[idx]] = [tracks[idx], tracks[idx - 1]];
			} else {
				const idx = tracks.findIndex(t => t.id === trackId);
				if (idx < tracks.length - 1) [tracks[idx], tracks[idx + 1]] = [tracks[idx + 1], tracks[idx]];
			}
		} catch (e) { reportError(e); }
	}

	// -----------------------------------------------------------------------
	// Playback
	// -----------------------------------------------------------------------

	async function playTrack(track: PlaylistTrackRow): Promise<void> {
		const idx = tracks.findIndex(t => t.id === track.id);
		if (idx < 0) return;
		const queue: QueueTrack[] = tracks.map(t => ({
			path: t.path,
			title: t.title,
			artist: t.artist,
			album: t.album,
			duration_secs: t.durationSecs,
			album_art_path: t.albumArtPath,
		}));
		await setQueue(queue, idx);
		const probe = await probeSong(track.path);
		const song: SongInfo = {
			path: track.path,
			title: track.title,
			artist: track.artist,
			album: track.album,
			album_art_path: track.albumArtPath,
			sample_rate: probe.sample_rate,
			bits_per_sample: probe.bits_per_sample,
			duration_secs: probe.duration_secs,
		};
		invoke('play', { song }).catch((e) => console.error('[Playlist] play failed:', e));
	}

	async function queueTrack(track: PlaylistTrackRow): Promise<void> {
		const probe = await probeSong(track.path);
		const song: SongInfo = {
			path: track.path,
			title: track.title,
			artist: track.artist,
			album: track.album,
			album_art_path: track.albumArtPath,
			sample_rate: probe.sample_rate,
			bits_per_sample: probe.bits_per_sample,
			duration_secs: probe.duration_secs,
		};
		invoke('queue_next', { song }).catch((e) => console.error('[Playlist] queue_next failed:', e));
	}

	// -----------------------------------------------------------------------
	// Export
	// -----------------------------------------------------------------------

	async function exportPlaylist(): Promise<void> {
		if (selectedId == null) return;
		try {
			const content = await invoke<string>('export_playlist', { id: selectedId });
			const name = await invoke<string>('export_playlist_name', { id: selectedId });
			const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
			const a = document.createElement('a');
			a.href = URL.createObjectURL(blob);
			a.download = `${name}.m3u8`;
			a.click();
			URL.revokeObjectURL(a.href);
		} catch (e) { reportError(e); }
	}

	// -----------------------------------------------------------------------
	// Helpers
	// -----------------------------------------------------------------------

	function reportError(e: unknown): void {
		const msg = e instanceof Error ? e.message : String(e);
		lastError = msg;
		setTimeout(() => { lastError = ''; }, 5000);
	}

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
			input.click();
		});
	}

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

	function trackLabel(t: PlaylistTrackRow): string {
		if (t.artist && t.title) return `${t.artist} — ${t.title}`;
		return t.title || t.path.split('/').pop() || t.path;
	}

	function isPlaying(t: PlaylistTrackRow): boolean {
		return t.path === playingPath;
	}

	// -----------------------------------------------------------------------
	// Init
	// -----------------------------------------------------------------------

	$effect(() => {
		loadPlaylists().then(() => {
			if (playlists.length > 0) selectPlaylist(playlists[0].id);
		});
	});
</script>

<section class="page">
	<!-- Sidebar: playlist list -->
	<LiquidGlass roundness={0} accent="#bef264" contrast="light">
		<aside class="sidebar">
		<div class="sidebar-header">
			<div class="sidebar-title">
				<Icon name="playlist" size={14} />
				<span>Playlists</span>
			</div>
			<div class="sidebar-new-dropdown">
				<button class="sidebar-new" type="button" onclick={() => createMenuOpen = !createMenuOpen} aria-label="New playlist" title="New playlist">
					<Icon name="plus" size={16} />
				</button>
				{#if createMenuOpen}
					<div class="dropdown-menu" role="menu">
						<button class="dropdown-item" type="button" role="menuitem" onclick={() => { createPlaylist(); createMenuOpen = false; }}>
							<Icon name="playlist" size={14} />
							<span>New playlist</span>
						</button>
						<button class="dropdown-item" type="button" role="menuitem" onclick={() => { createAndImportM3u8(); createMenuOpen = false; }}>
							<Icon name="file-text" size={14} />
							<span>Import m3u8</span>
						</button>
					</div>
				{/if}
			</div>
		</div>
		<nav class="pl-list">
			{#each playlists as pl (pl.id)}
				{@const active = selectedId === pl.id}
			<div
				class="pl-item"
				class:active={selectedId === pl.id}
				onclick={() => selectPlaylist(pl.id)}
			>
				<div class="pl-item-body">
					<div class="pl-item-name">
						{#if selectedId === pl.id}
							<input
								type="text"
								class="pl-name-input"
								value={selectedName}
								onblur={(e) => { renamePlaylist(pl.id, e.currentTarget.value); }}
								onkeydown={(e) => {
									if (e.key === 'Enter') e.currentTarget.blur();
									if (e.key === 'Escape') { e.currentTarget.value = selectedName; e.currentTarget.blur(); }
								}}
							/>
						{:else}
							<span class="pl-name-text">{pl.name}</span>
						{/if}
					</div>
					<span class="pl-item-meta">{pl.trackCount} track{pl.trackCount === 1 ? '' : 's'}</span>
				</div>
				<button
					type="button"
					class="pl-delete"
					onclick={(e) => { e.stopPropagation(); deletePlaylist(pl.id); }}
					aria-label="Delete playlist"
					title="Delete"
				>
					<Icon name="close" size={12} />
				</button>
			</div>
			{/each}
			{#if playlists.length === 0}
				<div class="sidebar-empty">No playlists yet</div>
			{/if}
		</nav>
	</aside>
	</LiquidGlass>

	<!-- Main: track list -->
	<main class="main">
		{#if selectedId == null}
			<div class="empty-state">
				<div class="empty-icon"><Icon name="playlist" size={40} /></div>
				<p class="empty-title">No playlist selected</p>
				<p class="empty-sub">Create a playlist or import an m3u8 file to get started.</p>
			</div>
		{:else if loading}
			<div class="empty-state">
				<p class="empty-sub">Loading…</p>
			</div>
		{:else if tracks.length === 0}
			<div class="empty-state">
				<div class="empty-icon"><Icon name="music" size={36} /></div>
				<p class="empty-title">This playlist is empty</p>
				<p class="empty-sub">Import an m3u8 file or add audio files.</p>
			</div>
		{:else}
			<div class="track-list">
				{#each tracks as track, i (track.id)}
					{@const artUrl = artSrc(track.albumArtPath)}
					<div class="track-row" class:playing={isPlaying(track)}>
						<div class="track-thumb">
							{#if artUrl}
								<img src={artUrl} alt="" class="thumb-img" />
							{:else}
								<div class="thumb-fallback"><Icon name="music" size={20} /></div>
							{/if}
						</div>

						<button class="track-main" type="button" onclick={() => playTrack(track)}>
							<div class="track-body">
								<div class="track-name">{track.title || track.path.split('/').pop() || 'Unknown'}</div>
								<div class="track-artist">{track.artist || '—'}</div>
							</div>
						</button>

						<span class="track-dur">{fmtDuration(track.durationSecs)}</span>
						<span class="track-format">{formatInfo(track)}</span>

						<div class="track-actions">
							<button class="t-act" onclick={() => queueTrack(track)} aria-label="Queue next" title="Queue next">
								<Icon name="skip-next" size={13} />
							</button>
							<button class="t-act" onclick={() => moveTrack(track.id, 'up')} aria-label="Move up" title="Move up">
								<Icon name="arrow-up" size={13} />
							</button>
							<button class="t-act" onclick={() => moveTrack(track.id, 'down')} aria-label="Move down" title="Move down">
								<Icon name="arrow-down" size={13} />
							</button>
							<button class="t-act danger" onclick={() => removeTrack(track.id)} aria-label="Remove" title="Remove">
								<Icon name="close" size={13} />
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</main>

	<!-- Import dialog overlay -->
	{#if importDialogOpen}
		<div class="overlay" role="dialog" aria-label="Import m3u8">
			<LiquidGlass roundness={16} accent="#bef264" contrast="light">
				<div class="dialog">
					<h3 class="dialog-title">Import m3u8 playlist</h3>
					<p class="dialog-hint">Choose an <code>.m3u8</code> file. Relative paths are automatically resolved against its directory.</p>

					<button type="button" class="dlg-btn" onclick={pickM3u8} disabled={importing}>
						{importFilePath ? 'Change file…' : 'Choose m3u8 file…'}
					</button>
					{#if importFilePath}
						<p class="file-chosen">{importFilePath}</p>
						<p class="dir-resolved">Base dir: <code>{importBaseDir || '(none — absolute paths only)'}</code></p>
					{/if}

					{#if importError}
						<p class="dialog-error">{importError}</p>
					{/if}

					<div class="dialog-actions">
						<button class="dlg-btn primary" type="button" onclick={doImport} disabled={!importFilePath || importing}>
							{importing ? 'Importing…' : 'Import'}
						</button>
						<button class="dlg-btn" type="button" onclick={() => { importDialogOpen = false; }} disabled={importing}>
							Cancel
						</button>
					</div>
					{#if importing}
						<div class="import-progress">
							<div class="spinner" aria-label="Importing playlist…"></div>
							<p>Importing playlist…</p>
						</div>
					{/if}
				</div>
			</LiquidGlass>
		</div>
	{/if}

	<!-- Error bar -->
	{#if lastError}
		<div class="error-bar" role="alert">{lastError}</div>
	{/if}
</section>

<!-- Toolbar (floating footer) -->
{#if selectedId != null}
	<footer class="toolbar">
		<span class="toolbar-name">{selectedName}</span>
		<span class="toolbar-meta">{trackCount} track{trackCount === 1 ? '' : 's'} · {fmtDuration(totalDuration)}</span>
		<div class="toolbar-actions">
			<button class="t-btn" type="button" onclick={openImportDialog}>
				<Icon name="plus" size={14} /> Import m3u8
			</button>
			<button class="t-btn" type="button" onclick={addFiles}>
				<Icon name="music" size={14} /> Add files
			</button>
			<button class="t-btn" type="button" onclick={exportPlaylist}>
				<Icon name="plus" size={14} /> Export
			</button>
		</div>
	</footer>
{/if}

<style>
	.page {
		display: flex;
		height: 100%;
		width: 100%;
		background: radial-gradient(circle at 20% -10%, var(--uto-ambient-tint), transparent 55%), var(--uto-bg);
		color: var(--uto-text);
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		overflow: hidden;
	}

	/* ── Sidebar ──────────────────────────────────────────────────────── */
	.sidebar {
		width: 240px;
		height: 100%;
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.sidebar-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px 12px;
		border-bottom: 1px solid var(--uto-glass-border);
	}
	.sidebar-title {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 12px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--uto-text-muted);
	}
	.sidebar-new {
		appearance: none;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		color: var(--uto-text-muted);
		width: 28px;
		height: 28px;
		border-radius: 8px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.18s, color 0.18s;
	}
	.sidebar-new:hover { background: rgba(190,242,100,0.12); color: var(--uto-accent-green, #bef264); }
	.sidebar-new-dropdown {
		position: relative;
	}
	.dropdown-menu {
		position: absolute;
		top: 100%;
		right: 0;
		margin-top: 4px;
		min-width: 160px;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		border: 1px solid var(--uto-glass-border);
		border-radius: 10px;
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			0 8px 24px var(--uto-glass-outer-shadow);
		padding: 4px;
		z-index: 100;
	}
	.dropdown-item {
		appearance: none;
		width: 100%;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border: none;
		border-radius: 7px;
		background: transparent;
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		text-align: left;
		cursor: pointer;
		transition: background 0.15s;
	}
	.dropdown-item:hover { background: rgba(190,242,100,0.1); }
	.pl-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.pl-list::-webkit-scrollbar { width: 6px; }
	.pl-list::-webkit-scrollbar-thumb { background: var(--uto-scrollbar-thumb); border-radius: 6px; }

	.pl-item {
		appearance: none;
		width: 100%;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 10px;
		border: none;
		border-radius: 10px;
		background: transparent;
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		text-align: left;
		cursor: pointer;
		transition: background 0.15s;
		margin-bottom: 2px;
	}
	.pl-item:hover { background: rgba(190,242,100,0.07); }
	.pl-item.active {
		background: rgba(190,242,100,0.12);
	}
	.pl-item-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.pl-item-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 500;
	}
	.pl-name-input {
		appearance: none;
		background: rgba(190,242,100,0.08);
		border: 1px solid rgba(190,242,100,0.25);
		border-radius: 5px;
		color: inherit;
		font: inherit;
		padding: 1px 6px;
		outline: none;
		width: 100%;
	}
	.pl-item-meta {
		font-size: 11px;
		color: var(--uto-text-faint);
	}
	.pl-delete {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-faint);
		cursor: pointer;
		width: 22px;
		height: 22px;
		border-radius: 6px;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: background 0.15s, color 0.15s, opacity 0.15s;
		flex-shrink: 0;
	}
	.pl-item:hover .pl-delete { opacity: 1; }
	.pl-delete:hover { background: rgba(252,165,165,0.1); color: #fca5a5; }
	.sidebar-empty {
		font-size: 12px;
		color: var(--uto-text-faint);
		padding: 16px 10px;
		text-align: center;
	}

	/* ── Main area ────────────────────────────────────────────────────── */
	.main {
		flex: 1;
		overflow-y: auto;
		padding: 12px 14px 80px;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.main::-webkit-scrollbar { width: 8px; }
	.main::-webkit-scrollbar-thumb { background: var(--uto-scrollbar-thumb); border-radius: 8px; }

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
		height: 100%;
		color: var(--uto-text-faint);
		text-align: center;
		padding: 40px;
	}
	.empty-icon { opacity: 0.35; margin-bottom: 4px; }
	.empty-title { font-size: 16px; color: var(--uto-text-muted); font-weight: 500; margin: 0; }
	.empty-sub { font-size: 13px; margin: 0; }

	.track-list {
		display: flex;
		flex-direction: column;
	}
	.track-row {
		position: relative;
		padding: 10px 14px;
		display: flex;
		align-items: center;
		gap: 12px;
		min-height: 60px;
		border-bottom: 1px solid var(--uto-glass-border);
		transition: background 0.15s;
	}
	.track-row:hover { background: rgba(190,242,100,0.04); }
	.track-row.playing { background: rgba(190,242,100,0.08); }
	.track-thumb {
		flex-shrink: 0;
		width: 48px;
		height: 48px;
		border-radius: 8px;
		overflow: hidden;
		background: rgba(190,242,100,0.08);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.thumb-img {
		width: 48px;
		height: 48px;
		object-fit: cover;
		display: block;
	}
	.thumb-fallback {
		color: var(--uto-accent-green, #bef264);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.track-main {
		flex: 1;
		min-width: 0;
		appearance: none;
		border: none;
		background: transparent;
		color: inherit;
		font-family: inherit;
		cursor: pointer;
		text-align: left;
		padding: 0;
	}
	.track-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
	.track-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--uto-text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.track-artist {
		font-size: 12px;
		color: var(--uto-text-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.track-dur {
		flex-shrink: 0;
		font-variant-numeric: tabular-nums;
		font-size: 11px;
		color: var(--uto-text-muted);
		min-width: 36px;
		text-align: right;
	}
	.track-format {
		flex-shrink: 0;
		font-size: 10px;
		font-weight: 500;
		font-variant-numeric: tabular-nums;
		color: var(--uto-text-faint);
		letter-spacing: 0.02em;
		white-space: nowrap;
		padding: 2px 8px;
		border-radius: 6px;
		background: rgba(190,242,100,0.06);
		border: 1px solid rgba(190,242,100,0.1);
	}
	.track-actions {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: 2px;
		opacity: 0;
		transition: opacity 0.18s;
	}
	.track-row:hover .track-actions,
	.track-row.playing .track-actions { opacity: 1; }
	.t-act {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		width: 26px;
		height: 26px;
		border-radius: 7px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.15s, color 0.15s;
	}
	.t-act:hover { background: rgba(190,242,100,0.1); color: var(--uto-text); }
	.t-act.danger:hover { color: #fca5a5; background: rgba(252,165,165,0.08); }

	/* ── Toolbar ──────────────────────────────────────────────────────── */
	.toolbar {
		position: absolute;
		bottom: 0;
		left: 240px;
		right: 0;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 10px 18px;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 -4px 24px var(--uto-glass-outer-shadow);
		border-top: 1px solid var(--uto-glass-border);
		z-index: 5;
	}
	.toolbar-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--uto-text-strong);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 200px;
	}
	.toolbar-meta {
		font-size: 11px;
		color: var(--uto-text-faint);
		flex-shrink: 0;
	}
	.toolbar-actions {
		margin-left: auto;
		display: flex;
		gap: 6px;
	}
	.t-btn {
		appearance: none;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		color: var(--uto-text);
		font-family: inherit;
		font-size: 12px;
		font-weight: 500;
		padding: 6px 10px;
		border-radius: 8px;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		gap: 5px;
		transition: background 0.15s, transform 0.15s;
	}
	.t-btn:hover { background: rgba(190,242,100,0.1); transform: translateY(-1px); }
	.t-btn:active { transform: scale(0.97); }

	/* ── Import dialog ────────────────────────────────────────────────── */
	.overlay {
		position: absolute;
		inset: 0;
		z-index: 20;
		background: rgba(0,0,0,0.45);
		backdrop-filter: blur(6px);
		-webkit-backdrop-filter: blur(6px);
		display: flex;
		align-items: center;
		justify-content: center;
		animation: fadeIn 0.15s ease-out;
	}
	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }

	.dialog {
		width: min(460px, 90vw);
		padding: 22px 24px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.dialog-title {
		font-size: 17px;
		font-weight: 600;
		margin: 0;
		color: var(--uto-text-strong);
	}
	.dialog-hint {
		font-size: 12px;
		color: var(--uto-text-muted);
		margin: 0;
	}
	.dialog-hint code {
		background: rgba(190,242,100,0.1);
		padding: 1px 6px;
		border-radius: 4px;
		font-size: 11px;
	}
	.file-chosen {
		font-size: 12px;
		color: var(--uto-accent-green, #bef264);
		word-break: break-all;
	}
	.dir-resolved {
		font-size: 12px;
		color: var(--uto-text-faint);
	}
	.dir-resolved code {
		background: rgba(190,242,100,0.1);
		padding: 1px 6px;
		border-radius: 4px;
		font-size: 11px;
	}
	.dialog-error {
		font-size: 12px;
		color: #fca5a5;
		margin: 0;
	}
	.dialog-actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
		margin-top: 4px;
	}
	.dlg-btn {
		appearance: none;
		border: 1px solid var(--uto-glass-border);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		font-weight: 500;
		padding: 8px 16px;
		border-radius: 10px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.dlg-btn:hover:not(:disabled) { background: rgba(190,242,100,0.1); }
	.dlg-btn.primary {
		background: rgba(190,242,100,0.14);
		border-color: rgba(190,242,100,0.28);
	}
	.dlg-btn.primary:hover:not(:disabled) { background: rgba(190,242,100,0.22); }
	.dlg-btn:disabled { opacity: 0.5; cursor: not-allowed; }

	.import-progress {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 12px 0 4px;
		color: var(--uto-text-muted);
		font-size: 12px;
	}
	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--uto-glass-border);
		border-top-color: var(--uto-accent-green, #bef264);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}
	@keyframes spin { to { transform: rotate(360deg); } }

	.error-bar {
		position: absolute;
		top: 10px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 30;
		font-size: 12px;
		color: #fca5a5;
		background: rgba(30,10,10,0.9);
		padding: 6px 14px;
		border-radius: 10px;
		border: 1px solid rgba(252,165,165,0.25);
		backdrop-filter: blur(12px);
	}

	/* ── Mobile ───────────────────────────────────────────────────────── */
	@media (max-width: 768px) {
		.sidebar { width: 100%; max-height: 200px; border-right: none; border-bottom: 1px solid var(--uto-glass-border); }
		.page { flex-direction: column; }
		.track-dur, .track-format { display: none; }
		.track-actions { opacity: 1; }
		.toolbar { left: 0; }
	}
</style>
