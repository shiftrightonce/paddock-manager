<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { api } from '../api.js';
import { guard, loadFarms, store, toast } from '../composables/store.js';
import { fmt } from '../composables/format.js';
import NewFarmDialog from '../components/NewFarmDialog.vue';
import FieldDialog from '../components/FieldDialog.vue';

const router = useRouter();
const farms = ref([]);
const fields = ref([]);
const loading = ref(true);
const editingFarm = ref(null);
const editingField = ref(null);
const showFarmDialog = ref(false);

async function load() {
  loading.value = true;
  await guard(async () => {
    if (!store.farms.length) await loadFarms();
    farms.value = store.farms;
    const all = await Promise.all((store.farms || []).map((f) => api.get(`/api/fields?farmId=${f.id}`)));
    fields.value = all.flat();
  });
  loading.value = false;
}

const fieldsByFarm = computed(() => {
  const map = new Map();
  for (const f of farms.value) map.set(f.id, fields.value.filter((x) => x.farm_id === f.id));
  return map;
});

const areaTotal = computed(() => fields.value.reduce((sum, f) => sum + (f.area_ha || 0), 0));

function editFarm(farm) {
  editingFarm.value = farm;
  showFarmDialog.value = true;
}

function addField(farm) {
  editingField.value = { farm_id: farm.id };
  editingFarm.value = null;
}

function editField(field) {
  editingField.value = field;
}

async function removeField(field) {
  if (!window.confirm(`Delete ${field.name}? Entries and tasks for this field are removed.`)) return;
  await guard(async () => {
    await api.del(`/api/fields/${field.id}`);
    toast('Field deleted', 'info');
    await load();
  });
}

async function removeFarm(farm) {
  if (!window.confirm(`Delete ${farm.name} and all of its fields?`)) return;
  await guard(async () => {
    await api.del(`/api/farms/${farm.id}`);
    toast('Farm deleted', 'info');
    await load();
  });
}

async function saved() {
  showFarmDialog.value = false;
  editingField.value = null;
  await load();
}

onMounted(load);
</script>

<template>
  <div class="stack">
    <div class="row row--between">
      <h2 class="section-title" style="margin: 0">Farms and fields</h2>
      <button class="btn btn--primary btn--sm" @click="showFarmDialog = true; editingFarm = null">
        Add a farm
      </button>
    </div>

    <div v-if="loading" class="skeleton" style="height: 220px" />

    <div v-else-if="!farms.length" class="card">
      <div class="card__body">
        <div class="empty">
          <div class="empty__title">No farms yet</div>
          Add a farm to start recording fields.
        </div>
      </div>
    </div>

    <template v-else>
      <div class="stat-grid">
        <div class="stat">
          <div class="stat__label">Farms</div>
          <div class="stat__value">{{ farms.length }}</div>
        </div>
        <div class="stat">
          <div class="stat__label">Fields</div>
          <div class="stat__value">{{ fields.length }}</div>
        </div>
        <div class="stat">
          <div class="stat__label">Total area</div>
          <div class="stat__value">{{ fmt(areaTotal) }}<span class="stat__unit">ha</span></div>
        </div>
      </div>

      <div v-for="farm in farms" :key="farm.id" class="card">
        <div class="card__body">
          <div class="row row--between row--top">
            <div>
              <h3 style="font-size: 16px">{{ farm.name }}</h3>
              <div class="small muted">{{ farm.location || 'No location set' }}</div>
            </div>
            <div class="row" style="gap: 6px">
              <button class="btn btn--sm" @click="editFarm(farm)">Edit</button>
              <button class="btn btn--sm" @click="addField(farm)">Add field</button>
            </div>
          </div>
          <div class="stat-grid" style="margin-top: 10px">
            <div class="stat stat--red">
              <div class="stat__label">Red</div>
              <div class="stat__value">{{ farm.redCount }}</div>
            </div>
            <div class="stat stat--amber">
              <div class="stat__label">Amber</div>
              <div class="stat__value">{{ farm.amberCount }}</div>
            </div>
            <div class="stat">
              <div class="stat__label">Fields</div>
              <div class="stat__value">{{ fieldsByFarm.get(farm.id)?.length || 0 }}</div>
            </div>
            <div class="stat">
              <div class="stat__label">Total Farm Area</div>
              <div class="stat__value">{{  fieldsByFarm.get(farm.id).reduce((sum, f) => sum + (f.area_ha || 0), 0).toFixed(2) }}</div>
            </div>
          </div>
        </div>

        <div v-if="fieldsByFarm.get(farm.id)?.length" class="link-list">
          <div v-for="field in fieldsByFarm.get(farm.id)" :key="field.id" class="field-row" style="border: 0; border-radius: 0">
            <div class="field-row__stripe" :class="`stripe-${field.status}`" />
            <RouterLink class="field-row__main" :to="`/fields/${field.id}`">
              <div class="row row--between">
                <span class="field-row__name">{{ field.name }}</span>
                <span class="chip" :class="`chip--${field.status}`">
                  {{ field.status === 'red' ? 'Resistance' : field.status === 'amber' ? 'Residual' : 'Clear' }}
                </span>
              </div>
              <div class="field-row__meta">
                <template v-if="field.area_ha != null">{{ fmt(field.area_ha) }} ha · </template>
                {{ field.soil_type || 'Soil type not set' }}
              </div>
            </RouterLink>
            <div class="field-row__chev field-row__actions">
              <button class="btn btn--ghost btn--sm" @click="editField(field)">Edit</button>
              <button class="btn btn--ghost btn--sm" @click="removeField(field)">Delete</button>
            </div>
          </div>
        </div>
        <div v-else class="card__foot">
          <span class="tiny muted">No fields yet. Use "Add field" to create one.</span>
        </div>
        <div class="card__foot">
          <button class="btn btn--ghost btn--sm" @click="removeFarm(farm)">Delete farm</button>
        </div>
      </div>
    </template>

    <NewFarmDialog
      v-if="showFarmDialog"
      :farm="editingFarm"
      @cancel="showFarmDialog = false"
      @saved="saved"
    />

    <FieldDialog
      v-if="editingField"
      :field="editingField.id ? editingField : null"
      :farm-id="editingField.farm_id"
      :soils="store.reference.options.soilTypes"
      @cancel="editingField = null"
      @saved="saved"
    />
  </div>
</template>