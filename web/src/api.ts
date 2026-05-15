export type Folder = { path: string; recursive: boolean; file_count: number };
export type BrowseEntry = { name: string; is_dir: boolean };
export type BrowseResp = { path: string; parent: string | null; entries: BrowseEntry[] };

export async function listFolders(): Promise<Folder[]> {
	const r = await fetch('/api/folders');
	if (!r.ok) throw new Error(await r.text());
	return r.json();
}

export async function addFolder(path: string, recursive: boolean): Promise<Folder> {
	const r = await fetch('/api/folders', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, recursive }),
	});
	if (!r.ok) throw new Error(await r.text());
	return r.json();
}

export async function removeFolder(path: string): Promise<void> {
	const r = await fetch('/api/folders', {
		method: 'DELETE',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path }),
	});
	if (!r.ok) throw new Error(await r.text());
}

export async function refreshFolder(path: string): Promise<{ file_count: number }> {
	const r = await fetch('/api/folders/refresh', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path }),
	});
	if (!r.ok) throw new Error(await r.text());
	return r.json();
}

export async function browse(path?: string): Promise<BrowseResp> {
	const u = path ? `/api/browse?path=${encodeURIComponent(path)}` : '/api/browse';
	const r = await fetch(u);
	if (!r.ok) throw new Error(await r.text());
	return r.json();
}
