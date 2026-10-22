<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import { RouterLink, RouterView, useRoute } from 'vue-router';
import {
  store,
  dismissToast,
  loadFarms,
  loadReference,
  restoreFarm,
  guard,
  toast,
  refreshBadge,
} from './composables/store.js';

const route = useRoute();
const ready = ref(false);
const bootError = ref('');

const headerTitle = computed(() => route.meta?.title || 'Paddock Manager');
const headerSub = computed(() => {
  if (route.name === 'field' || route.name === 'resistance' || route.name === 'rotation') {
    return store.fieldName || '';
  }
  if (route.name === 'dashboard') {
    const farm = store.farms.find((f) => f.id === store.currentFarmId);
    return farm ? farm.name : '';
  }
  return '';
});

const nav = [
  { to: '/', label: 'Farm', exact: true },
  { to: '/farms', label: 'Fields' },
  { to: '/tasks', label: 'Tasks' },
  { to: '/mixes', label: 'Mixes' },
  { to: '/export', label: 'Export' },
];

const badgeFor = (to) => (to === '/tasks' ? store.badge : 0);

function isActive(to, exact) {
  if (exact) return route.path === to;
  return route.path === to || route.path.startsWith(`${to}/`);
}

onMounted(async () => {
  const saved = restoreFarm();
  if (saved) store.currentFarmId = saved;
  await guard(async () => {
    await Promise.all([loadFarms(), loadReference(), refreshBadge()]);
    ready.value = true;
  });
  if (!ready.value) bootError.value = 'Could not reach the Paddock Manager API.';
});

watch(
  () => route.params.fieldId,
  async (id) => {
    if (!id) {
      store.fieldName = '';
      return;
    }
    await guard(async () => {
      const field = await fetch(`/api/fields/${id}`).then((r) => (r.ok ? r.json() : null));
      if (field) store.fieldName = field.field.name;
    });
  },
  { immediate: true },
);

function reload() {
  toast('Reloading', 'info', 1200);
  window.location.reload();
}
</script>

<template>
  <div class="app">
    <header class="app-header">
      <div class="app-header__bar">
        <button
          v-if="route.name !== 'dashboard'"
          class="btn btn--ghost btn--sm"
          style="color: #fff"
          @click="$router.back()"
        >
          Back
        </button>
        <div class="app-header__title">
          {{ headerTitle }}
          <span v-if="headerSub" class="app-header__sub">{{ headerSub }}</span>
        </div>
        <span v-if="store.badge" class="badge" :title="`${store.badge} items need attention`">
          {{ store.badge }}
        </span>
        <button class="btn btn--ghost btn--sm" style="color: #fff" @click="reload">Refresh</button>
      </div>
      <nav class="app-header__nav" aria-label="Main">
        <RouterLink
          v-for="item in nav"
          :key="item.to"
          :to="item.to"
          class="navlink"
          :class="{ 'router-link-active': isActive(item.to, item.exact) }"
        >
          {{ item.label }}
          <span v-if="badgeFor(item.to)" class="badge" style="margin-left: 5px">{{ badgeFor(item.to) }}</span>
        </RouterLink>
      </nav>
    </header>

    <main class="app-main">
      <div v-if="bootError" class="alert alert--red">
        <span class="alert__icon">!</span>
        <div>
          <strong>{{ bootError }}</strong>
          <div class="small" style="margin-top: 4px">
            Make sure the API is running (<span class="mono">npm run dev</span>) and try again.
          </div>
          <button class="btn btn--sm" style="margin-top: 8px" @click="reload">Retry</button>
        </div>
      </div>

      <template v-else-if="ready">
        <RouterView />
      </template>

      <div v-else class="stack" aria-busy="true" aria-label="Loading">
        <div class="skeleton" />
        <div class="skeleton" />
        <div class="skeleton" style="height: 120px" />
      </div>
    </main>

    <div class="toasts" role="status" aria-live="polite">
      <div
        v-for="t in store.toasts"
        :key="t.id"
        class="toast"
        :class="{ 'toast--error': t.kind === 'error', 'toast--warn': t.kind === 'warn' }"
        @click="dismissToast(t.id)"
      >
        <span>{{ t.message }}</span>
      </div>
    </div>
  </div>
</template>