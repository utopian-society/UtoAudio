<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { emit } from '@tauri-apps/api/event';
	import { scanLibrary } from '../lib/file-browser';
	import { AUDIO_EXTENSIONS } from '../lib/file-browser';
	import Icon from '../components/Icon.svelte';
	import { LiquidGlass } from '../lib/liquid-glass';
	import {
		appState,
		toggleExtension as toggleExt,
		isExtensionEnabled,
		setLyricFontSize,
	} from '../lib/store.svelte';

	// -----------------------------------------------------------------------
	// Backend serde types (camelCase args per Tauri's default convention)
	// -----------------------------------------------------------------------

	/** Mirrors `audio_core::tauri_api::EQBand`. */
	interface EQBand {
		freq_hz: number;
		gain_db: number;
	}
	/** Mirrors `audio_core::tauri_api::EqualizerPreset`. */
	interface EqualizerPreset {
		enabled: boolean;
		bands: EQBand[];
	}

	type CrossfadeCurve = 'equal_power' | 'linear' | 'square_root' | 's_curve';

	/** Mirrors `audio_core::tauri_api::CrossfadeConfig`. */
	interface CrossfadeConfig {
		enabled: boolean;
		duration_secs: number;
		curve: CrossfadeCurve;
	}

	const CROSSFADE_CURVES: { value: CrossfadeCurve; label: string }[] = [
		{ value: 'equal_power', label: 'Equal Power' },
		{ value: 'linear', label: 'Linear' },
		{ value: 'square_root', label: 'Square Root' },
		{ value: 's_curve', label: 'S-Curve' },
	];

	const EQ_FREQS_HZ: { value: number; label: string }[] = [
		{ value: 32, label: '32' },
		{ value: 64, label: '64' },
		{ value: 125, label: '125' },
		{ value: 250, label: '250' },
		{ value: 500, label: '500' },
		{ value: 1000, label: '1k' },
		{ value: 2000, label: '2k' },
		{ value: 4000, label: '4k' },
		{ value: 8000, label: '8k' },
		{ value: 16000, label: '16k' },
	];

	// -----------------------------------------------------------------------
	// Section state — collapsible cards
	// -----------------------------------------------------------------------

	let audioOpen = $state(true);
	let playbackOpen = $state(true);
	let eqOpen = $state(true);
	let libraryOpen = $state(true);
	let appearanceOpen = $state(true);

	function toggle(name: 'audioOpen' | 'playbackOpen' | 'eqOpen' | 'libraryOpen' | 'appearanceOpen'): void {
		switch (name) {
			case 'audioOpen': audioOpen = !audioOpen; break;
			case 'playbackOpen': playbackOpen = !playbackOpen; break;
			case 'eqOpen': eqOpen = !eqOpen; break;
			case 'libraryOpen': libraryOpen = !libraryOpen; break;
			case 'appearanceOpen': appearanceOpen = !appearanceOpen; break;
		}
	}

	// -----------------------------------------------------------------------
	// Audio output
	// -----------------------------------------------------------------------

	let highResMode = $state(false);
	let bitPerfect = $state(false);
	let four32Hz = $state(false);
	let sampleRatePreference = $state<number | ''>('');

	async function onHighResChange(v: boolean): Promise<void> {
		highResMode = v;
		try {
			await invoke('set_high_res_mode', { enabled: v });
		} catch (e) {
			reportError(e);
		}
	}
	async function onBitPerfectChange(v: boolean): Promise<void> {
		bitPerfect = v;
		try {
			await invoke('set_dap_bit_perfect_enabled', { enabled: v });
		} catch (e) {
			reportError(e);
		}
	}
	async function on432Change(v: boolean): Promise<void> {
		four32Hz = v;
		try {
			await invoke('set_432hz_tuning_enabled', { enabled: v });
		} catch (e) {
			reportError(e);
		}
	}

	// -----------------------------------------------------------------------
	// Playback — crossfade + volume
	// -----------------------------------------------------------------------

	let crossfadeEnabled = $state(false);
	let crossfadeDuration = $state(4);
	let crossfadeCurve = $state<CrossfadeCurve>('equal_power');
	let volume = $state(0.8);

	async function onCrossfadeChange(): Promise<void> {
		try {
			const config: CrossfadeConfig = {
				enabled: crossfadeEnabled,
				duration_secs: crossfadeDuration,
				curve: crossfadeCurve,
			};
			await invoke('set_crossfade', { config });
		} catch (e) {
			reportError(e);
		}
	}
	async function onVolumeChange(): Promise<void> {
		try {
			await invoke('set_volume', { volume });
		} catch (e) {
			reportError(e);
		}
	}

	// -----------------------------------------------------------------------
	// Equalizer — 10-band
	// -----------------------------------------------------------------------

	// Initial gains are flat (0 dB); the curve is symmetric — the slider's
	// value is captured straight into `GAIN_DB[i]`.
	let eqEnabled = $state(false);
	let eqGains = $state<number[]>(EQ_FREQS_HZ.map(() => 0));

	function resetEq(): void {
		eqGains = EQ_FREQS_HZ.map(() => 0);
		void pushEq();
	}

	async function pushEq(): Promise<void> {
		const bands: EQBand[] = eqGains.map((gain, i) => ({
			freq_hz: EQ_FREQS_HZ[i].value,
			gain_db: gain,
		}));
		const preset: EqualizerPreset = { enabled: eqEnabled, bands };
		try {
			await invoke('set_equalizer', { preset });
		} catch (e) {
			reportError(e);
		}
	}

	async function onEqToggleChange(v: boolean): Promise<void> {
		eqEnabled = v;
		await pushEq();
	}
	function onBandInput(i: number, e: Event): void {
		const target = e.target as HTMLInputElement;
		eqGains[i] = Number(target.value);
	}
	async function onBandCommit(): Promise<void> {
		await pushEq();
	}

	// -----------------------------------------------------------------------
	// Library — scan roots + extension filter
	// -----------------------------------------------------------------------

	// `scanRoots` lives in the SQLite-backed library index (the
	// `scan_roots` table in `library.sqlite`). The Settings page reads
	// and writes it via the `get_scan_roots` / `add_scan_root` /
	// `remove_scan_root` Tauri commands. Only the pending-input string
	// is local.
	let scanRoots = $state<string[]>([]);
	let pendingRoot = $state('');

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

	async function removeScanRoot(root: string): Promise<void> {
		try {
			await invoke('remove_scan_root', { path: root });
			await loadScanRoots();
		} catch (e) {
			reportError(e);
		}
	}

	// Load scan roots on mount.
	$effect(() => {
		void loadScanRoots();
	});

	function rescanNow(): void {
		// Real wiring: invoke the Rust `scan_library` command with the user's
		// currently-configured scan roots and the enabled-extension set,
		// then emit a `library:rescanned` event so the Library page refreshes
		// itself. Transient errors surface through the existing
		// `reportError()` channel; success shows a brief count summary.
		void runRescan();
	}

	let scanning = $state(false);
	let scanSummary = $state('');
	let scanSummaryTimer: ReturnType<typeof setTimeout> | null = null;

	function flashSummary(text: string): void {
		scanSummary = text;
		if (scanSummaryTimer != null) clearTimeout(scanSummaryTimer);
		scanSummaryTimer = setTimeout(() => {
			scanSummary = '';
			scanSummaryTimer = null;
		}, 4000);
	}

	async function runRescan(): Promise<void> {
		if (scanning) return;
		if (scanRoots.length === 0) {
			reportError('Add a scan directory before rescanning.');
			return;
		}
		scanning = true;
		try {
			// Normalise the extension set: prepend `.` if missing, lowercase.
			const extensions = Array.from(appState.enabledExtensions).map((ext) =>
				ext.startsWith('.') ? ext.toLowerCase() : `.${ext.toLowerCase()}`,
			);
			const results = await scanLibrary(scanRoots, extensions);
			const dirs = results.filter((r) => r.isDirectory).length;
			const files = results.length - dirs;
			flashSummary(
				`Scanned ${results.length} entr${results.length === 1 ? 'y' : 'ies'} (${dirs} folders · ${files} audio files)`,
			);
			await emit('library:rescanned', {
				count: results.length,
				roots: [...scanRoots],
			});
		} catch (e) {
			reportError(e);
		} finally {
			scanning = false;
		}
	}

	// -----------------------------------------------------------------------
	// Appearance — theme + lyric font size
	// -----------------------------------------------------------------------

	// Theme lives in the global store; the dropdown writes
	// `appState.theme`, and `App.svelte`'s `$effect` calls `applyTheme()`
	// to push the choice onto `<html data-theme>` so the CSS variables
	// flip across every page.
	let lyricFontSize = $state(36);

	// -----------------------------------------------------------------------
	// Error reporting
	// -----------------------------------------------------------------------

	let lastError = $state('');

	function reportError(e: unknown): void {
		const msg = e instanceof Error ? e.message : String(e);
		lastError = msg;
		// Auto-clear after 5 seconds so transient error popups don't linger.
		setTimeout(() => {
			lastError = '';
		}, 5000);
	}
</script>

<section class="page">
	<header class="page-header">
		<h1 class="page-title">Settings</h1>
		{#if lastError}
			<span class="error" role="alert">{lastError}</span>
		{/if}
	</header>

	<div class="cards">
		<!-- AUDIO OUTPUT -->
		<LiquidGlass roundness={18} accent="#bef264" contrast="light">
			<section class="card-inner">
				<button class="card-header" type="button" onclick={() => toggle('audioOpen')}>
					<span class="card-icon"><Icon name="speaker" size={18} /></span>
					<span class="card-title">Audio Output</span>
					<span class="chev"><Icon name={audioOpen ? 'chevron-down' : 'chevron-right'} size={12} /></span>
				</button>
				{#if audioOpen}
					<div class="card-body">
						<div class="row">
							<label for="sr-pref">Preferred sample rate</label>
							<select id="sr-pref" bind:value={sampleRatePreference}>
								<option value="">Auto</option>
								<option value={44100}>44.1 kHz</option>
								<option value={48000}>48 kHz</option>
								<option value={96000}>96 kHz</option>
								<option value={192000}>192 kHz</option>
								<option value={384000}>384 kHz</option>
							</select>
						</div>
						<div class="row">
							<label for="bp">Bit-perfect (DAP Internal)</label>
							{@render toggleSwitch({ checked: bitPerfect, onChange: onBitPerfectChange, id: 'bp' })}
						</div>
						<div class="row">
							<label for="hr">High-res mode</label>
							{@render toggleSwitch({ checked: highResMode, onChange: onHighResChange, id: 'hr' })}
						</div>
						<div class="row">
							<label for="f32">432 Hz tuning</label>
							{@render toggleSwitch({ checked: four32Hz, onChange: on432Change, id: 'f32' })}
						</div>
					</div>
				{/if}
			</section>
		</LiquidGlass>

		<!-- PLAYBACK -->
		<LiquidGlass roundness={18} accent="#bef264" contrast="light">
			<section class="card-inner">
				<button class="card-header" type="button" onclick={() => toggle('playbackOpen')}>
					<span class="card-icon"><Icon name="play" size={18} /></span>
					<span class="card-title">Playback</span>
					<span class="chev"><Icon name={playbackOpen ? 'chevron-down' : 'chevron-right'} size={12} /></span>
				</button>
				{#if playbackOpen}
					<div class="card-body">
						<div class="row">
							<label for="cf-enabled">Crossfade</label>
							{@render toggleSwitch({
								id: 'cf-enabled',
								checked: crossfadeEnabled,
								onChange: (v: boolean) => {
									crossfadeEnabled = v;
									onCrossfadeChange();
								},
							})}
						</div>
						<div class="row">
							<label for="cf-dur">Crossfade duration · {crossfadeDuration}s</label>
							<input
								id="cf-dur"
								class="slider"
								type="range"
								min="0"
								max="30"
								step="0.5"
								bind:value={crossfadeDuration}
								onchange={onCrossfadeChange}
								disabled={!crossfadeEnabled}
							/>
						</div>
						<div class="row">
							<label for="cf-curve">Crossfade curve</label>
							<select
								id="cf-curve"
								bind:value={crossfadeCurve}
								onchange={onCrossfadeChange}
								disabled={!crossfadeEnabled}
							>
								{#each CROSSFADE_CURVES as c (c.value)}
									<option value={c.value}>{c.label}</option>
								{/each}
							</select>
						</div>
						<div class="row">
							<label for="vol">Default volume · {Math.round(volume * 100)}%</label>
							<input
								id="vol"
								class="slider"
								type="range"
								min="0"
								max="1"
								step="0.01"
								bind:value={volume}
								onchange={onVolumeChange}
							/>
						</div>
					</div>
				{/if}
			</section>
		</LiquidGlass>

		<!-- EQUALIZER -->
		<LiquidGlass roundness={18} accent="#bef264" contrast="light">
			<section class="card-inner">
				<button class="card-header" type="button" onclick={() => toggle('eqOpen')}>
					<span class="card-icon"><Icon name="eq" size={18} /></span>
					<span class="card-title">Equalizer</span>
					<div class="eq-state">
						{@render toggleSwitch({
							id: 'eq-enabled',
							checked: eqEnabled,
							onChange: onEqToggleChange,
						})}
						<span class="chev"><Icon name={eqOpen ? 'chevron-down' : 'chevron-right'} size={12} /></span>
					</div>
				</button>
				{#if eqOpen}
					<div class="card-body">
						<div class="eq-bands">
							{#each EQ_FREQS_HZ as band, i (band.value)}
								<div class="eq-band">
									<span class="eq-val" class:zero={eqGains[i] === 0}>
										{eqGains[i] > 0 ? '+' : ''}{eqGains[i]}
									</span>
									<input
										class="slider eq-slider"
										type="range"
										min="-12"
										max="12"
										step="0.5"
										value={eqGains[i]}
										oninput={(e) => onBandInput(i, e)}
										onchange={onBandCommit}
										aria-label={`EQ ${band.label} Hz`}
									/>
									<span class="eq-freq">{band.label}</span>
								</div>
							{/each}
						</div>
						<div class="row">
							<button class="btn" type="button" onclick={resetEq}>Reset to flat</button>
						</div>
					</div>
				{/if}
			</section>
		</LiquidGlass>

		<!-- LIBRARY -->
		<LiquidGlass roundness={18} accent="#bef264" contrast="light">
			<section class="card-inner">
				<button class="card-header" type="button" onclick={() => toggle('libraryOpen')}>
					<span class="card-icon"><Icon name="library" size={18} /></span>
					<span class="card-title">Library</span>
					<span class="chev"><Icon name={libraryOpen ? 'chevron-down' : 'chevron-right'} size={12} /></span>
				</button>
				{#if libraryOpen}
					<div class="card-body">
						<div class="scan-roots">
							{#if scanRoots.length === 0}
								<p class="muted">No scan directories added</p>
							{:else}
								<ul class="root-list">
									{#each scanRoots as root (root)}
									<li class="root-row">
										<span class="root-icon"><Icon name="folder" size={14} /></span>
										<span class="root-path">{root}</span>
										<button
											class="btn ghost danger icon-only"
											type="button"
											onclick={() => removeScanRoot(root)}
											aria-label="Remove"
										><Icon name="close" size={14} /></button>
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
								<button class="btn" type="submit">Add</button>
							</form>
							<div class="rescan-row">
								<button
									class="btn primary"
									type="button"
									onclick={rescanNow}
									disabled={scanning}
								>
									{#if scanning}
										<Icon name="rescan" size={14} class="spin" />
										<span>Scanning…</span>
									{:else}
										<Icon name="rescan" size={14} />
										<span>Rescan now</span>
									{/if}
								</button>
								{#if scanSummary}
									<span class="scan-summary">{scanSummary}</span>
								{/if}
							</div>
						</div>

					<div class="ext-grid">
						{#each AUDIO_EXTENSIONS as ext (ext)}
							<label class="ext-chip" class:on={isExtensionEnabled(ext)}>
								<input
									type="checkbox"
									checked={isExtensionEnabled(ext)}
									onchange={() => toggleExt(ext)}
								/>
								{ext}
							</label>
						{/each}
					</div>
					</div>
				{/if}
			</section>
		</LiquidGlass>

		<!-- APPEARANCE -->
		<LiquidGlass roundness={18} accent="#bef264" contrast="light">
			<section class="card-inner">
				<button class="card-header" type="button" onclick={() => toggle('appearanceOpen')}>
					<span class="card-icon"><Icon name="appearance" size={18} /></span>
					<span class="card-title">Appearance</span>
					<span class="chev"><Icon name={appearanceOpen ? 'chevron-down' : 'chevron-right'} size={12} /></span>
				</button>
				{#if appearanceOpen}
					<div class="card-body">
						<div class="row">
							<label for="lys">Lyric font size · {lyricFontSize}px</label>
							<input
								id="lys"
								class="slider"
								type="range"
								min="20"
								max="64"
								step="2"
								bind:value={lyricFontSize}
								onchange={() => setLyricFontSize(lyricFontSize)}
							/>
						</div>
					</div>
				{/if}
			</section>
		</LiquidGlass>
	</div>
</section>

{#snippet toggleSwitch(props: { checked: boolean; onChange: (v: boolean) => void | Promise<void>; id: string; label?: string })}
	<button
		type="button"
		role="switch"
		aria-checked={props.checked}
		id={props.id}
		aria-label={props.label ?? props.id}
		class="toggle"
		class:on={props.checked}
		onclick={() => props.onChange(!props.checked)}
	>
		<span class="knob" aria-hidden="true"></span>
	</button>
{/snippet}

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
		width: 100%;
		background:
			radial-gradient(circle at 20% -10%, var(--uto-ambient-tint), transparent 55%),
			var(--uto-bg);
		color: var(--uto-text);
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: var(--uto-scrollbar-thumb) transparent;
	}
	.page::-webkit-scrollbar {
		width: 8px;
	}
	.page::-webkit-scrollbar-thumb {
		background: var(--uto-scrollbar-thumb);
		border-radius: 8px;
	}
	.page::-webkit-scrollbar-thumb:hover {
		background: var(--uto-scrollbar-thumb-hover);
	}

	/* --- Page header -------------------------------------------------------- */
	.page-header {
		display: flex;
		align-items: baseline;
		gap: 14px;
		padding: 20px 24px 12px;
	}
	.page-title {
		font-size: 26px;
		font-weight: 600;
		letter-spacing: -0.01em;
		color: var(--uto-text-strong);
		margin: 0;
	}
	.error {
		font-size: 12px;
		color: #fca5a5;
		background: rgba(252, 165, 165, 0.1);
		padding: 4px 10px;
		border-radius: 8px;
		border: 1px solid rgba(252, 165, 165, 0.22);
	}

	/* --- Cards ------------------------------------------------------------- */
	.cards {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 8px 24px 32px;
		max-width: 720px;
		width: 100%;
		box-sizing: border-box;
	}
	.card-inner {
		/* LiquidGlass provides the glass surface, border-radius, and overflow:hidden.
		   This inner wrapper only carries layout. */
		display: flex;
		flex-direction: column;
	}
	.card-header {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text);
		font-family: inherit;
		text-align: left;
		padding: 14px 18px;
		cursor: pointer;
		transition: background 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.card-header:hover {
		background: rgba(190, 242, 100, 0.04);
	}
	.card-icon {
		font-size: 18px;
		color: var(--uto-accent-yellow, #fef08a);
		display: flex;
		align-items: center;
	}
	.card-title {
		flex: 1;
		font-size: 15px;
		font-weight: 600;
		letter-spacing: 0.01em;
		color: var(--uto-text);
	}
	.chev {
		color: var(--uto-text-faint);
		font-size: 12px;
		display: flex;
		align-items: center;
	}
	.card-body {
		padding: 4px 18px 18px 18px;
		display: flex;
		flex-direction: column;
		gap: 14px;
		border-top: 1px solid var(--uto-glass-border);
	}

	/* --- Rows -------------------------------------------------------------- */
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding-top: 6px;
	}
	.row label {
		font-size: 13px;
		color: var(--uto-text);
		min-width: 0;
	}

	/* --- Form controls ----------------------------------------------------- */
	select {
		appearance: none;
		padding: 6px 28px 6px 10px;
		border-radius: 10px;
		border: 1px solid var(--uto-glass-border);
		background: var(--uto-surface);
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate));
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		cursor: pointer;
		outline: none;
		background-image: linear-gradient(45deg, transparent 50%, var(--uto-text-muted) 50%),
			linear-gradient(135deg, var(--uto-text-muted) 50%, transparent 50%);
		background-position: calc(100% - 14px) 50%, calc(100% - 9px) 50%;
		background-size: 5px 5px, 5px 5px;
		background-repeat: no-repeat;
		transition:
			border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			box-shadow 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	select:focus {
		border-color: rgba(190, 242, 100, 0.4);
		box-shadow: 0 0 0 3px rgba(190, 242, 100, 0.08);
	}
	select:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Range slider — lime track with yellow thumb. */
	.slider {
		appearance: none;
		flex: 1;
		height: 4px;
		border-radius: 999px;
		background: rgba(190, 242, 100, 0.25);
		outline: none;
		cursor: pointer;
		max-width: 280px;
	}
	.slider::-webkit-slider-thumb {
		appearance: none;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--uto-accent-yellow, #fef08a);
		border: 2px solid var(--uto-slider-thumb-border);
		box-shadow: 0 2px 8px rgba(254, 240, 138, 0.35);
		cursor: pointer;
		transition: transform 0.1s ease;
	}
	.slider::-webkit-slider-thumb:hover {
		transform: scale(1.1);
	}
	.slider::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--uto-accent-yellow, #fef08a);
		border: 2px solid var(--uto-slider-thumb-border);
		cursor: pointer;
	}
	.slider:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	/* --- Toggle switch ----------------------------------------------------- */
	.toggle {
		flex-shrink: 0;
		width: 36px;
		height: 22px;
		border-radius: 22px;
		border: 1px solid var(--uto-glass-border);
		background:
			linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur));
		position: relative;
		cursor: pointer;
		transition:
			background 0.2s cubic-bezier(0.22,1,0.36,1),
			border-color 0.2s cubic-bezier(0.22,1,0.36,1);
		padding: 0;
	}
	.toggle .knob {
		display: block;
		position: absolute;
		top: 50%;
		left: 2px;
		transform: translateY(-50%);
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--uto-text-faint);
		transition:
			transform 0.2s cubic-bezier(0.22,1,0.36,1),
			background 0.2s cubic-bezier(0.22,1,0.36,1);
	}
	.toggle.on {
		background: rgba(190, 242, 100, 0.28);
		border-color: rgba(190, 242, 100, 0.45);
	}
	.toggle.on .knob {
		transform: translateY(-50%) translateX(14px);
		background: var(--uto-accent-green, #bef264);
	}

	/* --- EQ ---------------------------------------------------------------- */
	.eq-state {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.eq-bands {
		display: flex;
		justify-content: space-between;
		gap: 4px;
		padding: 14px 4px 6px;
	}
	.eq-band {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
		flex: 1;
	}
	.eq-val {
		font-variant-numeric: tabular-nums;
		font-size: 11px;
		color: var(--uto-accent-green, #bef264);
		min-height: 14px;
	}
	.eq-val.zero {
		color: var(--uto-text-faint);
	}
	.eq-slider {
		-webkit-appearance: slider-vertical;
		appearance: slider-vertical;
		writing-mode: vertical-lr;
		direction: rtl;
		width: 8px;
		height: 110px;
		flex: none;
		max-width: none;
	}
	.eq-freq {
		font-size: 10px;
		color: var(--uto-text-faint);
		letter-spacing: 0.02em;
	}

	/* --- Library ----------------------------------------------------------- */
	.scan-roots {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.root-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.root-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		border-radius: 10px;
		background: var(--uto-input-bg);
		border: 1px solid var(--uto-input-border);
	}
	.root-icon {
		font-size: 14px;
		opacity: 0.7;
	}
	.root-path {
		flex: 1;
		font-size: 13px;
		color: var(--uto-text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.add-root {
		display: flex;
		gap: 6px;
	}
	.add-root-input {
		flex: 1;
		appearance: none;
		padding: 7px 10px;
		border-radius: 10px;
		border: 1px solid var(--uto-input-border);
		background: var(--uto-input-bg);
		color: var(--uto-text);
		font-family: inherit;
		font-size: 13px;
		outline: none;
	}
	.add-root-input:focus {
		border-color: rgba(190, 242, 100, 0.4);
	}

	.ext-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
		gap: 6px;
	}
	.ext-chip {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 10px;
		border-radius: 10px;
		border: 1px solid var(--uto-input-border);
		background: var(--uto-input-bg);
		font-size: 12px;
		color: var(--uto-text-faint);
		cursor: pointer;
		user-select: none;
		transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
	}
	.ext-chip input {
		accent-color: var(--uto-accent-green, #bef264);
	}
	.ext-chip.on {
		color: var(--uto-text-strong);
		border-color: rgba(190, 242, 100, 0.28);
		background: rgba(190, 242, 100, 0.08);
	}

	/* --- Misc -------------------------------------------------------------- */
	.btn {
		appearance: none;
		border: 1px solid var(--uto-glass-border);
		background:
			linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur));
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
		transition:
			background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1),
			border-color 0.18s cubic-bezier(0.22,1,0.36,1),
			transform 0.18s cubic-bezier(0.22,1,0.36,1),
			box-shadow 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.btn:hover {
		background: rgba(190, 242, 100, 0.08);
		color: var(--uto-text-strong);
		border-color: rgba(190, 242, 100, 0.15);
		transform: translateY(-1px);
		box-shadow: 0 4px 16px rgba(190, 242, 100, 0.06);
	}
	.btn:active {
		transform: scale(0.97);
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		transform: none;
		box-shadow: none;
	}
	.btn.primary {
		background: rgba(190, 242, 100, 0.14);
		color: var(--uto-text-strong);
		border-color: rgba(190, 242, 100, 0.28);
	}
	.btn.primary:hover {
		background: rgba(190, 242, 100, 0.22);
		border-color: rgba(190, 242, 100, 0.35);
	}
	.btn.ghost {
		border: none;
		background: transparent;
		backdrop-filter: none;
		-webkit-backdrop-filter: none;
	}
	.btn.ghost.danger:hover {
		color: #fca5a5;
		background: rgba(252, 165, 165, 0.1);
	}
	.btn.icon-only {
		padding: 4px;
		border-radius: 6px;
		background: transparent;
		border: none;
		backdrop-filter: none;
		-webkit-backdrop-filter: none;
	}

	.rescan-row {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.scan-summary {
		font-size: 12px;
		color: var(--uto-accent-green, #bef264);
		background: rgba(190, 242, 100, 0.08);
		padding: 4px 10px;
		border-radius: 8px;
		border: 1px solid rgba(190, 242, 100, 0.18);
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
	:global(.spin) {
		animation: spin 0.8s linear infinite;
	}

	.muted {
		color: var(--uto-text-faint);
		font-size: 12px;
	}

	/* --- Mobile ----------------------------------------------------------- */
	@media (max-width: 768px) {
		.cards {
			padding: 8px 14px 24px;
		}
		.eq-bands {
			gap: 2px;
		}
		.eq-freq {
			font-size: 9px;
		}
	}
</style>
