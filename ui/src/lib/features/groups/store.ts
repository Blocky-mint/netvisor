import { writable } from 'svelte/store';
import { api } from '../../shared/utils/api';
import type { Group } from '$lib/features/groups/types/base';
import { utcTimeZoneSentinel, uuidv4Sentinel } from '$lib/shared/utils/formatting';
import { getServices } from '../services/store';

export const groups = writable<Group[]>([]);

export async function getGroups() {
	return await api.request<Group[]>('/groups', groups, (groups) => groups, { method: 'GET' });
}

export async function createGroup(data: Group) {
	const result = await api.request<Group, Group[]>(
		'/groups',
		groups,
		(group, current) => [...current, group],
		{ method: 'POST', body: JSON.stringify(data) }
	);

	if (result?.success) {
		await getServices();
	}

	return result;
}

export async function updateGroup(data: Group) {
	const result = await api.request<Group, Group[]>(
		`/groups/${data.id}`,
		groups,
		(updatedGroup, current) => current.map((g) => (g.id === data.id ? updatedGroup : g)),
		{ method: 'PUT', body: JSON.stringify(data) }
	);

	if (result?.success) {
		await getServices();
	}

	return result;
}

export async function deleteGroup(id: string) {
	const result = await api.request<void, Group[]>(
		`/groups/${id}`,
		groups,
		(_, current) => current.filter((g) => g.id !== id),
		{ method: 'DELETE' }
	);

	if (result?.success) {
		await getServices();
	}

	return result;
}

export function createEmptyGroupFormData(): Group {
	return {
		id: uuidv4Sentinel,
		name: '',
		description: '',
		service_bindings: [],
		created_at: utcTimeZoneSentinel,
		updated_at: utcTimeZoneSentinel,
		group_type: 'NetworkPath'
	};
}
