<script lang="ts">
	import { listFolders, removeFolder, refreshFolder, type Folder } from './api';

	let { onbrowse }: { onbrowse: (path: string) => void } = $props();

	let folders = $state<Folder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function load() {
		try { folders = await listFolders(); error = null; }
		catch (e) { error = String(e); }
		finally { loading = false; }
	}
	load();

	async function onRemove(path: string) {
		if (!confirm(`Remove ${path}?`)) return;
		await removeFolder(path);
		await load();
	}
	async function onRefresh(path: string) {
		await refreshFolder(path);
		await load();
	}

	export async function reload() { await load(); }

	function basename(p: string): string {
		const parts = p.split('/').filter(Boolean);
		return parts[parts.length - 1] ?? p;
	}
</script>

{#if loading}
	<p>Loading…</p>
{:else if error}
	<p class="err">{error}</p>
{:else}
	<ul>
		{#each folders as f (f.path)}
			<li>
				<button class="name" onclick={() => onbrowse(f.path)} title="Browse files">
					{basename(f.path)} <span class="chev">▸</span>
				</button>
				<button class="icon" title="Refresh count" onclick={() => onRefresh(f.path)}>↻</button>
				<span class="count">{f.file_count} files</span>
				{#if f.recursive}<span class="tag">recursive</span>{/if}
				<span class="path" title={f.path}>{f.path}</span>
				<button class="icon" title="Remove" onclick={() => onRemove(f.path)}>✕</button>
			</li>
		{/each}
	</ul>
{/if}

<style>
	ul { list-style: none; padding: 0; margin: 0; }
	li {
		display: grid;
		grid-template-columns: auto auto auto auto 1fr auto;
		gap: 0.75rem;
		align-items: center;
		padding: 0.45rem 0;
		border-bottom: 1px solid #eee;
	}
	.name {
		background: none;
		border: none;
		padding: 0;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
		color: #111;
	}
	.name:hover { color: #000; }
	.chev { color: #999; font-weight: normal; }
	.count { font-variant-numeric: tabular-nums; color: #666; font-size: 0.85rem; white-space: nowrap; }
	.tag { font-size: 0.7rem; color: #888; background: #f3f3f3; padding: 0.05rem 0.4rem; border-radius: 3px; white-space: nowrap; }
	.path {
		color: #999;
		font-family: ui-monospace, monospace;
		font-size: 0.8rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.icon { background: none; border: 1px solid transparent; padding: 0.15rem 0.35rem; cursor: pointer; color: #777; font-size: 0.9rem; }
	.icon:hover { color: #111; border-color: #ddd; }
	.err { color: #c33; }
</style>
