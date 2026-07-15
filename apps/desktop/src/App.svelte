<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import NowPlaying from './pages/NowPlaying.svelte';
	import Playlist from './pages/Playlist.svelte';
	import Library from './pages/Library.svelte';
	import Settings from './pages/Settings.svelte';
	import Icon from './components/Icon.svelte';
	import Logo from './components/Logo.svelte';
	import { LiquidGlass } from './lib/liquid-glass';
	import { rehydrateSettings } from './lib/store.svelte';

	type Page = 'now-playing' | 'playlist' | 'library' | 'settings';

	interface Tab {
		id: Page;
		label: string;
		icon: IconIconName;
	}

	import type { IconName as IconIconName } from './components/Icon.svelte';

	let currentPage = $state<Page>('now-playing');

	const appWindow = getCurrentWindow();

	async function closeWindow(): Promise<void> {
		await appWindow.close();
	}
	async function minimizeWindow(): Promise<void> {
		await appWindow.minimize();
	}

	const tabs: Tab[] = [
		{ id: 'now-playing', label: 'Now Playing', icon: 'music' },
		{ id: 'playlist', label: 'Playlist', icon: 'playlist' },
		{ id: 'library', label: 'Library', icon: 'library' },
		{ id: 'settings', label: 'Settings', icon: 'gear' },
	];

	$effect(() => {
		rehydrateSettings();
	});
</script>

<div class="app-shell">
	<div class="titlebar" data-tauri-drag-region>
		<div class="titlebar-left">
			<Logo size={44} />
			<span class="titlebar-text">UtoAudio</span>
		</div>
		<div class="titlebar-controls">
			<button type="button" class="titlebar-btn" onclick={minimizeWindow} aria-label="Minimize">
				<Icon name="minimize" size={16} />
			</button>
			<button type="button" class="titlebar-btn" onclick={closeWindow} aria-label="Close">
				<Icon name="close" size={16} />
			</button>
		</div>
	</div>

	<div class="body">
		<nav class="sidebar">
			<LiquidGlass roundness={12} accent="#bef264" contrast="light">
				<div class="sidebar-inner">
					{#each tabs as tab (tab.id)}
						<button
							type="button"
							class="tab"
							class:active={currentPage === tab.id}
							onclick={() => (currentPage = tab.id)}
							aria-label={tab.label}
						>
							<Icon name={tab.icon} size={18} />
							<span class="tab-label">{tab.label}</span>
						</button>
					{/each}
				</div>
			</LiquidGlass>
		</nav>

		<main class="page-area">
			{#if currentPage === 'now-playing'}
				<NowPlaying />
			{:else if currentPage === 'playlist'}
				<Playlist />
			{:else if currentPage === 'library'}
				<Library />
			{:else if currentPage === 'settings'}
				<Settings />
			{/if}
		</main>
	</div>
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100%;
		width: 100%;
		background:
			radial-gradient(circle at 20% -10%, var(--uto-ambient-tint), transparent 55%),
			var(--uto-bg);
		color: var(--uto-text);
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		overflow: hidden;
	}

	.titlebar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 56px;
		padding: 0 12px;
		flex-shrink: 0;
		background:
			linear-gradient(135deg, var(--uto-glass-gradient-start), var(--uto-glass-gradient-end));
		backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
		box-shadow:
			inset 0 1px 0 var(--uto-rim-light),
			inset 0 -1px 0 var(--uto-glass-inset-bottom),
			0 4px 16px var(--uto-glass-outer-shadow);
		border-bottom: 1px solid var(--uto-glass-border);
		user-select: none;
	}
	.titlebar-left {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.titlebar-text {
		font-size: 13px;
		font-weight: 600;
		color: var(--uto-text);
		letter-spacing: 0.02em;
	}
	.titlebar-controls {
		display: flex;
		gap: 4px;
	}
	.titlebar-btn {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		width: 28px;
		height: 28px;
		border-radius: 6px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition:
			background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.titlebar-btn:hover {
		background: rgba(190, 242, 100, 0.08);
		color: var(--uto-text-strong);
	}

	.body {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.sidebar {
		width: 200px;
		flex-shrink: 0;
		padding: 12px 10px;
		display: flex;
		flex-direction: column;
	}
	.sidebar-inner {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px;
	}
	.tab {
		appearance: none;
		border: none;
		background: transparent;
		color: var(--uto-text-muted);
		font-family: inherit;
		font-size: 13px;
		font-weight: 500;
		padding: 10px 12px;
		border-radius: 10px;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 10px;
		text-align: left;
		transition:
			background 0.18s cubic-bezier(0.22,1,0.36,1),
			color 0.18s cubic-bezier(0.22,1,0.36,1);
	}
	.tab:hover {
		background: rgba(190, 242, 100, 0.06);
		color: var(--uto-text);
	}
	.tab.active {
		background: rgba(190, 242, 100, 0.12);
		color: var(--uto-text-strong);
	}
	.tab-label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.page-area {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}

	@media (max-width: 768px) {
		.body {
			flex-direction: column-reverse;
		}
		.sidebar {
			width: 100%;
			flex-shrink: 0;
			padding: 6px 8px;
		}
		.sidebar-inner {
			flex-direction: row;
			justify-content: space-around;
			gap: 2px;
			padding: 4px;
		}
		.tab {
			flex-direction: column;
			gap: 2px;
			padding: 6px 8px;
			font-size: 10px;
		}
		.tab-label {
			font-size: 10px;
		}
	}
</style>