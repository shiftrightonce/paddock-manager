<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { api } from '../api.js';
import { guard, loadFarms, refreshBadge, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';
import TaskDialog from '../components/TaskDialog.vue';

const route = useRoute();
const router = useRouter();

const fieldId = computed(() => Number(route.params.fieldId));
const detail = ref(null);
const fields = ref([]);
const loading = ref(true);
const showSchedule = ref(false);

const field = computed(() => detail.value?.field || null);
const resistance = computed(() => detail.value?.resistance || []);
const water = computed(() => detail.value?.water || null);
const lastResidual = computed(() => detail.value?.lastResidual || null);
const otherFields = computed(() => fields.value.filter((f) => f.farm_id === field.value?.farm_id));

async function load() {
  loading.value = true;
  await guard(async () => {
    if (!store.farms.length) await loadFarms();
    const [d, list] = await Promise.all([
      api.get(`/api/fields/${fieldId.value}`),
      api.get(`/api/fields?farmId=${field.value?.farm_id || ''}`),
    ]);
    detail.value = d;
    fields.value = list;
  });
  loading.value = false;
}

async function confirm(rec) {
  await guard(async () => {
    await api.put(`/api/resistance/${rec.id}`, {
      ...rec,
      confirmed_on: new Date().toISOString().slice(0, 10),
    });
    toast('Marked as confirmed', 'info');
    await load();
    await refreshBadge();
  });
}

async function remove(rec) {
  if (!window.confirm(`Delete the ${rec.weed_species} resistance record for ${rec.product}?`)) return;
  await guard(async () => {
    await api.del(`/api/resistance/${rec.id}`);
    toast('Record deleted', 'info');
    await load();
    await refreshBadge();
  });
}

async function record() {
  const product = window.prompt('Product name');
  if (!product) return;
  const weed = window.prompt('Weed species');
  if (!weed) return;
  const group = window.prompt('MOA group (optional)') || '';
  await guard(async () => {
    await api.post(`/api/fields/${fieldId.value}/resistance`, {
      weed_species: weed,
      product,
      moa_group: group,
      source: 'operator',
    });
    toast('Resistance recorded', 'info');
    await load();
    await refreshBadge();
  });
}

onMounted(load);
</script>

<template>
  <div class="stack">
    <div class="page-head">
      <button class="btn btn--ghost btn--sm" @click="router.push(`/fields/${fieldId}`)">‹ {{ field?.name || 'Field' }}</button>
      <h1 style="font-size: 20px">Weed resistance - {{ field?.name }}</h1>
      <div class="small muted">{{ field?.farm_name }}</div>
    </div>

    <div v-if="loading" class="skeleton" style="height: 240px" />

    <template v-else-if="field">
      <div
        class="alert"
        :class="resistance.length ? 'alert--red' : lastResidual ? 'alert--amber' : 'alert--green'"
      >
        <span class="alert__icon">{{ resistance.length ? '!' : 'i' }}</span>
        <div>
          <template v-if="resistance.length">
            <strong>{{ resistance.length }} resistance record{{ resistance.length === 1 ? '' : 's' }}.</strong>
            This field is red. Choose a different MOA group.
          </template>
          <template v-else-if="lastResidual">
            <strong>No resistance recorded.</strong>
            {{ lastResidual.residuals.join(', ') }} applied {{ lastResidual.daysSince }} days ago.
            Check the label plantback interval before sowing.
          </template>
          <template v-else>
            <strong>No resistance and no active residual.</strong> Field status is green.
          </template>
        </div>
      </div>

      <div class="card">
        <div class="card__head">Records</div>
        <div class="card__body card__body--flush">
          <div v-if="!resistance.length" class="empty">
            <div class="empty__title">Nothing recorded</div>
            Add a record when a product fails on this field.
          </div>
          <div v-for="rec in resistance" :key="rec.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe stripe-red" />
            <div class="field-row__main" style="cursor: default">
              <div class="row row--between">
                <span class="field-row__name">
                  {{ rec.weed_species }}
                  <span class="chip chip--red">{{ rec.product || 'unspecified' }}</span>
                  <span v-if="rec.moa_group" class="chip chip--plain">MOA {{ rec.moa_group }}</span>
                </span>
                <div class="row" style="gap: 6px">
                  <button v-if="!rec.confirmed_on" class="btn btn--sm" @click="confirm(rec)">Confirm</button>
                  <button class="btn btn--ghost btn--sm" @click="remove(rec)">Delete</button>
                </div>
              </div>
              <div class="field-row__meta">
                <template v-if="rec.active">Active {{ rec.active }} · </template>
                <template v-if="rec.confirmed_on">confirmed {{ rec.confirmed_on }} · </template>
                <template v-else>unconfirmed · </template>
                {{ rec.source || 'no source' }}
                <template v-if="rec.notes"> · {{ rec.notes }}</template>
              </div>
            </div>
          </div>
        </div>
        <div class="card__foot">
          <button class="btn btn--sm btn--block" @click="record">Record a resistance failure</button>
        </div>
      </div>

      <div class="card">
        <div class="card__head">Fields in {{ field.farm_name }}</div>
        <div class="card__body card__body--flush">
          <div v-for="f in otherFields" :key="f.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe" :class="`stripe-${f.status}`" />
            <RouterLink class="field-row__main" :to="`/fields/${f.id}/resistance`">
              <div class="row row--between">
                <span class="field-row__name">{{ f.name }}</span>
                <span class="chip" :class="`chip--${f.status}`">
                  {{ f.status === 'red' ? 'Resistance' : f.status === 'amber' ? 'Residual' : 'Clear' }}
                </span>
              </div>
              <div class="field-row__meta">{{ f.resistanceCount }} record{{ f.resistanceCount === 1 ? '' : 's' }}</div>
            </RouterLink>
            <div class="field-row__chev">›</div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card__body">
          <div class="row row--between">
            <span class="strong small">Schedule a scouting task</span>
            <button class="btn btn--sm" @click="showSchedule = true">Schedule</button>
          </div>
          <div class="tiny muted">
            Recording resistance is a one-off event. Scheduling regular scouting catches the next
            failure before it costs a season.
          </div>
        </div>
      </div>

      <TaskDialog
        v-if="showSchedule"
        :farms="store.farms"
        :fields="fields"
        :preset-farm-id="field.farm_id"
        :preset-field-id="fieldId"
        @cancel="showSchedule = false"
        @saved="showSchedule = false; refreshBadge()"
      />
    </template>
  </div>
</template>