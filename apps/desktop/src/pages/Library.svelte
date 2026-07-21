<script lang="ts">
	import { invoke, convertFileSrc } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import type { UnlistenFn } from '@tauri-apps/api/event';
import {
		scanDirectory,
		listAudioFiles,
		isAudioFile,
		AUDIO_EXTENSIONS,
	} from '../lib/file-browser';
	import type { FileEntry } from '../lib/file-browser';
	import Icon from '../components/Icon.svelte';
	import type { IconName } from '../components/Icon.svelte';
	import { LiquidGlass } from '../lib/liquid-glass';
import { setQueue, playQueueIndex, type QueueTrack } from '../lib/playback.svelte';

	/** Mirrors `audio_core::tauri_api::SongInfo`. Subset needed for `play`. */
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

	/** Probe a file for native sample rate / bit depth before playback. */
	async function probeSong(path: string): Promise<{ sample_rate?: number; bits_per_sample?: number; duration_secs?: number }> {
		try {
			const info = await invoke<{ sample_rate: number; channels: number; duration_secs: number; bits_per_sample?: number }>('probe_audio_file', { path });
			return {
				sample_rate: info.sample_rate,
				bits_per_sample: info.bits_per_sample,
				duration_secs: info.duration_secs,
			};
		} catch (e) {
			console.warn('[Library] probe_audio_file failed:', e);
			return {};
		}
	}

	// -----------------------------------------------------------------------
	// Reactive page state
	// -----------------------------------------------------------------------

	/** Currently-displayed directory (empty = the scan roots listing). */
	let currentPath = $state('');
	/** Entries in the current directory (mixed files + folders). */
	let entries = $state<FileEntry[]>([]);
	/** User-controlled search query (filters entries by filename, case-insensitive). */
	let searchQuery = $state('');

	// `scanRoots` lives in the SQLite-backed library index (the
	// `scan_roots` table in `library.sqlite`). The Library page reads
	// and writes it via the `get_scan_roots` / `add_scan_root` /
	// `remove_scan_root` Tauri commands.

	// Track-load promise: prevents flashes between route-in vs entries-in.
	let loading = $state(false);

	// -----------------------------------------------------------------------
	// Scan root management (SQLite-backed)
	// -----------------------------------------------------------------------

	let scanRoots = $state<string[]>([]);
	let pendingRoot = $state('');
	let lastError = $state('');

	async function loadScanRoots(): Promise<void> {
		try {
			scanRoots = await invoke<string[]>('get_scan_roots');
		} catch (e) {
			reportError(e);
		}
	}

	async function addScanRoot(): Promise<void> {
		const trimmed = pendingRoot.trim();
		if (!trimmed) return;
		try {
			await invoke('add_scan_root', { path: trimmed });
			pendingRoot = '';
			await loadScanRoots();
		} catch (e) {
			reportError(e);
		}
	}

	async function removeScanRoot(path: string): Promise<void> {
		try {
			await invoke('remove_scan_root', { path });
			await loadScanRoots();
		} catch (e) {
			reportError(e);
		}
	}

	function reportError(e: unknown): void {
		const msg = e instanceof Error ? e.message : String(e);
		lastError = msg;
		setTimeout(() => {
			lastError = '';
		}, 5000);
	}

	// -----------------------------------------------------------------------
	// Initial load + navigation
	// -----------------------------------------------------------------------

/** Breadcrumb segments for the path bar — each clickable. */
const breadcrumbs = $derived(buildBreadcrumbs(currentPath));

	function buildBreadcrumbs(path: string): { label: string; path: string }[] {
		if (!path) return [{ label: 'Library', path: '' }];
		const parts = path.split('/').filter(Boolean);
		const segments: { label: string; path: string }[] = [
			{ label: 'Library', path: '' },
		];
		let acc = '';
		for (const part of parts) {
			acc += `/${part}`;
			segments.push({ label: part, path: acc });
		}
		return segments;
	}

	async function enterDirectory(path: string): Promise<void> {
		console.log('[Library] enterDirectory:', path || '(roots)');
		loading = true;
		searchQuery = '';
		currentPath = path;
		try {
			if (path === '') {
				// At the top level, surface the configured scan roots as
				// clickable folder cards so the grid isn't empty — clicking
				// one descends into it via playEntry → enterDirectory.
				entries = scanRoots.map((r) => ({
					name: r.split('/').filter(Boolean).pop() || r,
					path: r,
					isDirectory: true,
				}));
			} else {
				entries = await scanDirectory(path, AUDIO_EXTENSIONS);
			}
		} catch (e) {
			reportError(e);
		} finally {
			loading = false;
		}
	}

	async function leaveDirectory(): Promise<void> {
		await enterDirectory('');
	}

	/** Navigate to a specific breadcrumb segment. */
	async function navigateTo(path: string): Promise<void> {
		await enterDirectory(path);
	}

	// Keep the roots-level grid in sync with the store: if the user adds or
	// removes a scan root from Settings while we're sitting at the top
	// level, refresh the folder cards so the change is immediately visible.
	$effect(() => {
		void scanRoots;
		if (currentPath === '') {
			entries = scanRoots.map((r) => ({
				name: r.split('/').filter(Boolean).pop() || r,
				path: r,
				isDirectory: true,
			}));
		}
	});

	// Load the roots view once on mount + listen for the `library:rescanned`
	// event emitted by Settings → "Rescan now". The scan roots themselves
	// live in the SQLite-backed library index, so we only need to re-scan
	// whatever directory we're currently in to refresh the grid against
	// the latest filesystem state.
	$effect(() => {
		// Mount-only effect: load scan roots once, enter the roots view, and
		// subscribe to `library:rescanned` for the lifetime of the page.
		// Deliberately reads NO reactive state synchronously — otherwise the
		// effect would re-run on every `currentPath` change and reset the
		// user back to the roots view mid-navigation.
		let mounted = true;
		let unlistenPromise: Promise<UnlistenFn> | null = null;
		loadScanRoots().finally(() => {
			if (!mounted) return;
			void enterDirectory('');
		});

		// Settings.svelte emits `library:rescanned` after a successful scan.
		unlistenPromise = listen<{ count: number; roots: string[] }>(
			'library:rescanned',
			() => {
				if (!mounted) return;
				void enterDirectory(currentPath);
			},
		);

		return () => {
			mounted = false;
			if (unlistenPromise) unlistenPromise.then((un) => un());
		};
	});

	// -----------------------------------------------------------------------
	// Search + filtering
	// -----------------------------------------------------------------------

	/** Current entries filtered by the user's search query. */
	const visibleEntries = $derived(
		filterByQuery(
			entries.filter((e) => e.isDirectory || isAudioFile(e.name)),
			searchQuery,
		),
	);

	// Load album art in batches when entries change
	$effect(() => {
		if (visibleEntries.length > 0) loadAlbumArts(visibleEntries);
	});

	function filterByQuery(items: FileEntry[], query: string): FileEntry[] {
		const q = query.trim().toLowerCase();
		if (!q) return items;
		return items.filter((e) => e.name.toLowerCase().includes(q));
	}

	// -----------------------------------------------------------------------
	// Album art — loaded in batches of 50 to avoid OOM kills.
	// -----------------------------------------------------------------------

	let artUrls = $state<Record<string, string>>({});

	async function loadAlbumArts(entries: FileEntry[]): Promise<void> {
		const paths = entries
			.map(e => e.albumArtPath)
			.filter((p): p is string => !!p);
		if (paths.length === 0) return;
		const TOTAL = paths.length;
		const BATCH_SIZE = 50;
		for (let i = 0; i < TOTAL; i += BATCH_SIZE) {
			const batch = paths.slice(i, i + BATCH_SIZE);
			await Promise.allSettled(batch.map(async (artPath) => {
				try { artUrls[artPath] = convertFileSrc(artPath); } catch {}
			}));
			console.debug(`Library album art ${Math.min(i + BATCH_SIZE, TOTAL)}/${TOTAL}`);
			await new Promise(r => setTimeout(r, 0));
		}
	}

	function artSrc(albumArtPath: string | null | undefined): string | null {
		if (!albumArtPath) return null;
		return artUrls[albumArtPath] ?? null;
	}

	// -----------------------------------------------------------------------
	// Playback + "add to playlist"
	// -----------------------------------------------------------------------

	async function playEntry(entry: FileEntry): Promise<void> {
		console.log('[Library] playEntry:', entry.name, entry.isDirectory ? '(dir)' : '(file)', entry.path);
		if (entry.isDirectory) {
			void enterDirectory(entry.path);
			return;
		}
		if (!isAudioFile(entry.name)) {
			console.warn('[Library] playEntry: not an audio file:', entry.name);
			return;
		}
		// Build the playback queue from all audio files in the current
		// folder (non-recursive, alphanumerically sorted — `entries` is
		// already sorted by `sort_entries` on the Rust side, directories
		// first then files by lowercase name). This gives the "play through
		// the folder" context the Now Playing page auto-advances within.
		const folderAudio = entries
			.filter((e) => !e.isDirectory && isAudioFile(e.name))
			.map((e): QueueTrack => ({
				path: e.path,
				title: e.title ?? e.name.replace(/\.[^/.]+$/, ''),
				album_art_path: e.albumArtPath,
			}));
		const startIndex = folderAudio.findIndex((t) => t.path === entry.path);

		const probe = await probeSong(entry.path);
		let meta: { title?: string; artist?: string; album?: string } = {};
		try {
			meta = await invoke<{ title?: string; artist?: string; album?: string }>('get_song_metadata', { path: entry.path });
		} catch (e) {
			console.warn('[Library] get_song_metadata failed:', e);
		}
		const song: SongInfo = {
			path: entry.path,
			title: entry.title || meta.title || entry.name.replace(/\.[^/.]+$/, ''),
			artist: meta.artist,
			album: meta.album,
			album_art_path: entry.albumArtPath,
			sample_rate: probe.sample_rate,
			bits_per_sample: probe.bits_per_sample,
			duration_secs: probe.duration_secs,
		};

		// Populate the queue with metadata-enriched tracks so the queue
		// viewer shows titles/artists, not just filenames.
		if (folderAudio.length > 1 && startIndex >= 0) {
			// Enrich queue entries with metadata (best-effort, non-blocking
			// — the playing track uses the metadata we already fetched).
			const enriched = folderAudio.map((t) => ({ ...t }));
			enriched[startIndex] = {
				...enriched[startIndex],
				title: song.title,
				artist: song.artist,
				album: song.album,
				duration_secs: song.duration_secs,
			};
			await setQueue(enriched, startIndex);
		}

		console.log('[Library] invoking play with:', song);
		invoke('play', { song })
			.then(() => {
				console.log('[Library] play succeeded for:', entry.path);
			})
			.catch((e) => {
				console.error('[Library] play failed:', e);
				reportError(e);
			});
	}

	async function queueEntry(entry: FileEntry): Promise<void> {
		if (entry.isDirectory) return;
		if (!isAudioFile(entry.name)) return;
		const probe = await probeSong(entry.path);
		let meta: { title?: string; artist?: string; album?: string } = {};
		try {
			meta = await invoke<{ title?: string; artist?: string; album?: string }>('get_song_metadata', { path: entry.path });
		} catch {}
		const song: SongInfo = {
			path: entry.path,
			title: entry.title || meta.title || entry.name.replace(/\.[^/.]+$/, ''),
			artist: meta.artist,
			album: meta.album,
			album_art_path: entry.albumArtPath,
			sample_rate: probe.sample_rate,
			bits_per_sample: probe.bits_per_sample,
			duration_secs: probe.duration_secs,
		};
		invoke('queue_next', { song })
			.then(() => {
				console.log('[Library] queue_next succeeded for:', entry.path);
			})
			.catch((e) => {
				console.error('[Library] queue_next failed:', e);
				reportError(e);
			});
	}

	/** Add all audio files in a folder (recursive) to a new playlist. */
	async function addFolderToPlaylist(folderPath: string): Promise<void> {
		try {
			const files = await listAudioFiles(folderPath);
			if (files.length === 0) {
				reportError('No audio files found in this folder');
				return;
			}
			const folderName = folderPath.split('/').filter(Boolean).pop() || 'Folder';
			const playlistId = await invoke<number>('create_playlist', { name: folderName });
			const paths = files.map(f => f.path);
			await invoke('add_tracks_to_playlist', { playlistId, paths });
			console.log('[Library] added', paths.length, 'files to playlist', folderName);
		} catch (e) {
			reportError(e);
		}
	}

	/** Show all audio files under the current path (across sub-folders). */
	let allAudio = $state<FileEntry[]>([]);
	let showAllAudio = $state(false);
	async function showAllFiles(): Promise<void> {
		loading = true;
		showAllAudio = true;
		try {
			allAudio = await listAudioFiles(currentPath);
			entries = allAudio;
		} catch (e) {
			reportError(e);
		} finally {
			loading = false;
		}
	}
	function backToTree(): void {
		showAllAudio = false;
		enterDirectory(currentPath);
	}

	// -----------------------------------------------------------------------
	// Formatting helpers
	// -----------------------------------------------------------------------

	function formatSize(bytes?: number): string {
		if (bytes == null) return '';
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
		if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
	}
	/** Format the audio file's technical metadata for the far-right badge:
	 *  extension · bit-depth · sample-rate. */
	function formatInfo(entry: FileEntry): string {
		if (entry.isDirectory) return '';
		const ext = entry.name.split('.').pop()?.toUpperCase() ?? '';
		const parts: string[] = [ext];
		if (entry.bitsPerSample) parts.push(`${entry.bitsPerSample}bit`);
		if (entry.sampleRate) {
			const khz = (entry.sampleRate / 1000).toFixed(1).replace(/\.0$/, '');
			parts.push(`${khz}kHz`);
		}
		return parts.join(' · ');
	}
	function iconFor(entry: FileEntry): IconName {
		return entry.isDirectory ? 'folder' : 'music';
	}
</script>

<section class="page">
	<header class="header">
		<div class="bar">
			<nav class="breadcrumbs" aria-label="Path">
				{#each breadcrumbs as crumb, i (crumb.path + i)}
					<button
						class="crumb"
						class:leaf={i === breadcrumbs.length - 1}
						type="button"
						onclick={() => navigateTo(crumb.path)}
					>
						{crumb.label}
					</button>
					{#if i < breadcrumbs.length - 1}
						<span class="sep" aria-hidden="true">/</span>
					{/if}
				{/each}
			</nav>

			<div class="search-wrap">
				<span class="search-icon" aria-hidden="true"><Icon name="search" size={14} /> </span>
				<input
					class="search"
					type="search"
					placeholder="Search this folder…"
					bind:value={searchQuery}
					aria-label="Search"
				/>
			</div>

			<div class="view-actions">
				{#if showAllAudio}
					<button class="btn" type="button" onclick={backToTree}>
						<Icon name="arrow-up" size={13} /> Tree
					</button>
				{:else}
					<button class="btn" type="button" onclick={showAllFiles}>
						<Icon name="rescan" size={13} /> Show all files
					</button>
				{/if}
			</div>
		</div>
		{#if lastError}
			<div class="error-bar" role="alert">{lastError}</div>
		{/if}
	</header>

	<div class="content">
		<LiquidGlass roundness={0} accent="#bef264" contrast="light">
			<aside class="sidebar">
			<div class="sidebar-section">
				<div class="sidebar-title">Scan directories</div>
				{#if scanRoots.length === 0}
					<p class="sidebar-empty">No directories yet</p>
				{:else}
					<ul class="roots">
						{#each scanRoots as root (root)}
							<li class="root-item">
								<button
									class="root-btn"
									type="button"
									onclick={() => enterDirectory(root)}
									title="Open"
								>
									<span class="root-icon"><Icon name="folder" size={14} /> </span>
									<span class="root-path">{root}</span>
								</button>
								<button
									class="root-remove"
									type="button"
									onclick={() => removeScanRoot(root)}
									aria-label="Remove"
								>
									<Icon name="close" size={12} />
								</button>
							</li>
						{/each}
					</ul>
				{/if}
				<form class="add-root" onsubmit={(e) => { e.preventDefault(); addScanRoot(); }}>
					<input
						class="add-root-input"
						type="text"
						bind:value={pendingRoot}
						placeholder="/path/to/music"
						aria-label="Add scan directory"
					/>
					<button class="btn primary" type="submit">Add</button>
				</form>
			</div>
		</aside>
		</LiquidGlass>

		<main class="main">
			{#if loading}
				<div class="prompt">Loading…</div>
			{:else if visibleEntries.length === 0}
				<div class="prompt">
					<div class="prompt-icon"><Icon name="library" size={32} /> </div>
					<p>{searchQuery ? 'No matches.' : 'This folder is empty.'}</p>
				</div>
			{:else}
				<div class="rows">
					{#each visibleEntries as entry (entry.path)}
						<div
							class="row-inner"
							class:dir={entry.isDirectory}
						>
								<div class="row-thumb">
									{#if entry.isDirectory}
										<div class="thumb-icon"><Icon name="folder" size={24} /></div>
									{:else}
										{@const artUrl = artSrc(entry.albumArtPath)}
										{#if artUrl}
											<img src={artUrl} alt="" class="thumb-img" />
										{:else}
											<div class="thumb-icon fallback"><Icon name="music" size={24} /></div>
										{/if}
									{/if}
								</div>

							<button
								class="row-main"
								type="button"
								onclick={() => playEntry(entry)}
								oncontextmenu={(e) => {
									if (entry.isDirectory) return;
									e.preventDefault();
									queueEntry(entry);
								}}
								aria-label={
									entry.isDirectory
										? `Open ${entry.name}`
										: `Play ${entry.title ?? entry.name}`
								}
							>
								<div class="row-body">
									<div class="row-name">{entry.title ?? entry.name}</div>
									<div class="row-meta">
										{#if entry.isDirectory}
											Folder
										{:else}
											{formatSize(entry.size)}
										{/if}
									</div>
								</div>
							</button>

							{#if !entry.isDirectory}
								<span class="row-format">{formatInfo(entry)}</span>
							{/if}

								<div class="row-actions">
									{#if entry.isDirectory}
										<button
											class="row-add"
											type="button"
											onclick={(e) => {
												e.stopPropagation();
												void addFolderToPlaylist(entry.path);
											}}
											aria-label="Add folder to playlist"
											title="Add all audio files in this folder to a playlist"
										><Icon name="plus" size={14} /></button>
									{:else}
										<button
											class="row-add"
											type="button"
											onclick={(e) => {
												e.stopPropagation();
												queueEntry(entry);
											}}
											aria-label="Add to playlist"
											title="Add to playlist"
										><Icon name="plus" size={14} /></button>
									{/if}
								</div>
							</div>
					{/each}
				</div>
			{/if}
		</main>
	</div>
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
		padding: 14px 18px 12px;
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 8px 32px var(--uto-glass-outer-shadow);
		border-bottom: 1px solid var(--uto-glass-border);
	}
	.error-bar {
		margin-top: 8px;
		font-size: 12px;
		color: #fca5a5;
		background: rgba(252, 165, 165, 0.1);
		padding: 4px 10px;
		border-radius: 8px;
		border: 1px solid rgba(252, 165, 165, 0.22);
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.breadcrumbs {
		display: flex;
		align-items: center;
		gap: 4px;
		min-width: 0;
		flex: 1;
		overflow: hidden;
	}
	.crumb {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		font-family: inherit;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		padding: 5px 8px;
		border-radius: 8px;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1);
		white-space: nowrap;
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.crumb:hover {
		background: rgba(190, 242, 100, 0.08);
		color: var(--uto-text-strong);
	}
	.crumb.leaf {
		color: var(--uto-text-strong);
		background: rgba(190, 242, 100, 0.1);
	}
	.crumb.leaf:hover {
		color: var(--uto-text-strong);
		background: rgba(190, 242, 100, 0.18);
	}
	.sep {
		color: var(--uto-text-faint);
		font-size: 13px;
	}

	.search-wrap {
		position: relative;
		display: flex;
		align-items: center;
		flex-shrink: 0;
	}
	.search-icon {
		position: absolute;
		left: 10px;
		color: var(--uto-text-faint);
		font-size: 15px;
		pointer-events: none;
	}
	.search {
		appearance: none;
		width: 260px;
		padding: 7px 12px 7px 30px;
		border-radius: 10px;
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
		outline: none;
		transition: border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			background 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.search:focus {
		border-color: rgba(190, 242, 100, 0.25);
		background: linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
	}

	.view-actions {
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

	/* --- Content layout ----------------------------------------------------- */
	.content {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.sidebar {
		width: 240px;
		height: 100%;
		flex-shrink: 0;
		padding: 14px 12px;
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.sidebar::-webkit-scrollbar {
		width: 8px;
	}
	.sidebar::-webkit-scrollbar-thumb {
		background: var(--uto-scrollbar-thumb);
		border-radius: 8px;
	}
	.sidebar::-webkit-scrollbar-thumb:hover {
		background: var(--uto-scrollbar-thumb-hover);
	}
	.sidebar-section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.sidebar-title {
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--uto-text-faint);
		padding-left: 4px;
	}
	.sidebar-empty {
		font-size: 13px;
		color: var(--uto-text-faint);
		margin: 0;
		padding: 4px;
	}
	.roots {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.root-item {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 2px;
		border-radius: 8px;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.root-item:hover {
		background: rgba(190, 242, 100, 0.08);
	}
	.root-btn {
		flex: 1;
		appearance: none;
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		text-align: left;
		padding: 6px 6px;
		border-radius: 6px;
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
	}
	.root-btn:hover {
		color: var(--uto-text-strong);
	}
	.root-icon {
		font-size: 14px;
	}
	.root-path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.root-remove {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-faint);
		cursor: pointer;
		padding: 4px 6px;
		border-radius: 6px;
		font-size: 11px;
	}
	.root-remove:hover {
		color: #fca5a5;
		background: rgba(252, 165, 165, 0.1);
	}
	.add-root {
		margin-top: 6px;
		display: flex;
		gap: 4px;
	}
	.add-root-input {
		flex: 1;
		appearance: none;
		padding: 6px 8px;
		border-radius: 8px;
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
		font-size: 12px;
		outline: none;
		transition: border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			background 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.add-root-input:focus {
		border-color: rgba(190, 242, 100, 0.25);
	}

	/* --- Main grid ---------------------------------------------------------- */
	.main {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.main::-webkit-scrollbar {
		width: 8px;
	}
	.main::-webkit-scrollbar-thumb {
		background: var(--uto-scrollbar-thumb);
		border-radius: 8px;
	}
	.main::-webkit-scrollbar-thumb:hover {
		background: var(--uto-scrollbar-thumb-hover);
	}

	.prompt {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
		height: 100%;
		color: var(--uto-text-faint);
	}
	.prompt-icon {
		font-size: 36px;
		opacity: 0.4;
	}
	.prompt p {
		margin: 0;
		font-size: 14px;
	}

	.rows {
		display: flex;
		flex-direction: column;
	}
	.row-inner {
		position: relative;
		padding: 10px 14px;
		display: flex;
		gap: 12px;
		align-items: center;
		min-height: 60px;
		border-bottom: 1px solid var(--uto-glass-border);
		transition: background 0.15s;
	}
	.row-inner:hover { background: rgba(190,242,100,0.04); }
	.row-inner.dir .row-name {
		color: var(--uto-text-strong);
	}

	.row-thumb {
		flex-shrink: 0;
		width: 48px;
		height: 48px;
		border-radius: 8px;
		overflow: hidden;
		background: rgba(190, 242, 100, 0.08);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.thumb-icon {
		color: var(--uto-accent-green, #bef264);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.thumb-icon.fallback {
		color: var(--uto-text-faint);
	}
	.thumb-img {
		width: 48px;
		height: 48px;
		object-fit: cover;
		display: block;
	}

	.row-main {
		display: flex;
		gap: 10px;
		align-items: center;
		flex: 1;
		min-width: 0;
		appearance: none;
		border: none;
		background: transparent;
		color: inherit;
		font-family: inherit;
		cursor: pointer;
		padding: 0;
		text-align: left;
	}
	.row-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.row-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--uto-text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.row-meta {
		font-size: 12px;
		color: var(--uto-text-muted);
	}

	.row-format {
		flex-shrink: 0;
		font-size: 10px;
		font-weight: 500;
		font-variant-numeric: tabular-nums;
		color: var(--uto-text-faint);
		letter-spacing: 0.02em;
		white-space: nowrap;
		padding: 2px 8px;
		border-radius: 6px;
		background: rgba(190, 242, 100, 0.06);
		border: 1px solid rgba(190, 242, 100, 0.1);
	}

	.row-actions {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.row-add {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-faint);
		font-size: 18px;
		font-weight: 500;
		width: 30px;
		height: 30px;
		border-radius: 8px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.row-add:hover {
		background: rgba(190, 242, 100, 0.18);
		color: var(--uto-accent-green, #bef264);
	}

	/* --- Mobile -------------------------------------------------------------- */
	@media (max-width: 768px) {
		.bar {
			flex-wrap: wrap;
		}
		.search {
			width: auto;
			flex: 1;
		}
		.sidebar {
			width: 100%;
			border-right: none;
			border-top: 1px solid var(--uto-glass-border);
			max-height: 260px;
		}
		.content {
			flex-direction: column-reverse;
		}
	}
</style>
