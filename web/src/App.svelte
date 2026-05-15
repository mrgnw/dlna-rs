<script lang="ts">
	import { listFolders, removeFolder, refreshFolder, type Folder } from './api';
	import AddFolderModal from './AddFolderModal.svelte';

	let folders = $state<Folder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let showAdd = $state(false);

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
	async function onAdded() {
		showAdd = false;
		await load();
	}
</script>

<main>
	<header>
		<h1>dlna-rs</h1>
	</header>
	{#if loading}
		<p>Loading…</p>
	{:else if error}
		<p class="err">{error}</p>
	{:else}
		<ul>
			{#each folders as f}
				<li>
					<span class="path">{f.path}</span>
					<button title="Refresh count" onclick={() => onRefresh(f.path)}>↻</button>
					<span class="count">{f.file_count} files</span>
					{#if f.recursive}<span class="tag">recursive</span>{/if}
					<button title="Remove" onclick={() => onRemove(f.path)}>✕</button>
				</li>
			{/each}
		</ul>
		<button class="add" onclick={() => showAdd = true}>+ add folder…</button>
	{/if}
	{#if showAdd}
		<AddFolderModal onclose={() => showAdd = false} onadded={onAdded} />
	{/if}
</main>

<style>
	main { max-width: 720px; margin: 2rem auto; font-family: ui-sans-serif, system-ui, sans-serif; padding: 0 1rem; }
	header { display: flex; align-items: baseline; justify-content: space-between; }
	h1 { font-size: 1.25rem; margin: 0 0 1rem; }
	ul { list-style: none; padding: 0; margin: 0; }
	li { display: flex; align-items: center; gap: 0.75rem; padding: 0.4rem 0; border-bottom: 1px solid #eee; }
	.path { flex: 1; font-family: ui-monospace, monospace; font-size: 0.9rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.count { font-variant-numeric: tabular-nums; color: #666; font-size: 0.85rem; }
	.tag { font-size: 0.7rem; color: #888; background: #f3f3f3; padding: 0.05rem 0.4rem; border-radius: 3px; }
	button { background: none; border: 1px solid transparent; padding: 0.2rem 0.4rem; cursor: pointer; color: #555; }
	button:hover { color: #000; border-color: #ddd; }
	.add { margin-top: 1rem; border: 1px solid #ddd; background: #fafafa; }
	.err { color: #c33; }
</style>
