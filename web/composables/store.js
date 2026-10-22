import { reactive, computed } from 'vue';
import { api } from '../api.js';

/**
 * Shared app state: farms, reference data, current farm, toasts.
 * Views read from here and call refresh*() after they mutate something.
 */
export const store = reactive({
  loading: false,
  farms: [],
  reference: {
    chemicals: [],
    crops: [],
    fertilisers: [],
    options: {
      soilTypes: [],
      irrigationMethods: [],
      weedDensityUnits: [],
      soilMoistureUnits: [],
      harvestUnits: [],
      rateUnits: [],
      strengthUnits: [],
      moaDescriptions: {},
    },
  },
  currentFarmId: null,
  /** Field name shown in the header while a field route is active. */
  fieldName: '',
  /** Count of needs-attention items across all farms, drives the badge. */
  badge: 0,
  toasts: [],
});

let toastSeq = 0;

export function toast(message, kind = 'info', ms = 4000) {
  const id = ++toastSeq;
  store.toasts.push({ id, message, kind });
  if (ms > 0) setTimeout(() => dismissToast(id), ms);
  return id;
}

export function dismissToast(id) {
  const i = store.toasts.findIndex((t) => t.id === id);
  if (i !== -1) store.toasts.splice(i, 1);
}

/** Wrap an async action with error reporting so every view handles failures. */
export async function guard(fn, { errorMessage = 'Something went wrong' } = {}) {
  try {
    return await fn();
  } catch (err) {
    toast(err.message || errorMessage, 'error', 6000);
    return undefined;
  }
}

export async function loadReference() {
  const [chemicals, crops, fertilisers, options] = await Promise.all([
    api.get('/api/reference/chemicals'),
    api.get('/api/reference/crops'),
    api.get('/api/reference/fertilisers'),
    api.get('/api/reference/options'),
  ]);
  store.reference.chemicals = chemicals;
  store.reference.crops = crops;
  store.reference.fertilisers = fertilisers;
  store.reference.options = options;
}

export async function loadFarms() {
  store.farms = await api.get('/api/farms');
  if (store.currentFarmId && !store.farms.some((f) => f.id === store.currentFarmId)) {
    store.currentFarmId = null;
  }
  if (!store.currentFarmId && store.farms.length) {
    store.currentFarmId = store.farms[0].id;
  }
  return store.farms;
}

/** Refresh the cross-farm needs-attention badge count. */
export async function refreshBadge() {
  try {
    const dash = await api.get('/api/dashboard');
    store.badge = dash.badge || 0;
  } catch {
    /* the header badge is not worth surfacing an error for */
  }
}

export async function setCurrentFarm(id) {
  store.currentFarmId = id ? Number(id) : null;
  if (typeof window !== 'undefined') {
    const key = 'paddock.lastFarm';
    if (store.currentFarmId) window.localStorage.setItem(key, String(store.currentFarmId));
    else window.localStorage.removeItem(key);
  }
}

export function restoreFarm() {
  if (typeof window === 'undefined') return null;
  const raw = window.localStorage.getItem('paddock.lastFarm');
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export const currentFarm = computed(
  () => store.farms.find((f) => f.id === store.currentFarmId) || null,
);

export const hasAnyFarm = computed(() => store.farms.length > 0);