<script setup>
import { computed, ref, watch } from 'vue';
import { api, links } from '../api.js';
import { store, guard, toast } from '../composables/store.js';
import { fmt, todayIso, IRRIGATION_LABEL } from '../composables/format.js';
import ModalDialog from './ModalDialog.vue';
import PlantingForm from './PlantingForm.vue';
import TankMixEditor from './TankMixEditor.vue';

const props = defineProps({
  fieldId: { type: Number, required: true },
  fieldName: { type: String, default: '' },
  entry: { type: Object, default: null },
  mixes: { type: Array, default: () => [] },
});
const emit = defineEmits(['saved', 'cancel', 'changed-mixes']);

const isEdit = computed(() => Boolean(props.entry?.id));
const busy = ref(false);
const errors = ref([]);
const plantingRef = ref(null);

const type = ref(props.entry?.type || 'chemical_application');
const entryDate = ref(props.entry?.entry_date || todayIso());
const notes = ref(props.entry?.notes || '');

// per-type form state
const soilTest = ref({ ...blank('soil_test'), ...(props.entry?.type === 'soil_test' ? props.entry.data : {}) });
const moisture = ref({ ...blank('soil_moisture'), ...(props.entry?.type === 'soil_moisture' ? props.entry.data : {}) });
const scouting = ref({ ...blank('weed_scouting'), ...(props.entry?.type === 'weed_scouting' ? props.entry.data : {}) });
const spray = ref({ ...blank('chemical_application'), ...(props.entry?.type === 'chemical_application' ? props.entry.data : {}) });
const harvest = ref({ ...blank('harvest'), ...(props.entry?.type === 'harvest' ? props.entry.data : {}) });
const water = ref({ ...blank('water'), ...(props.entry?.type === 'water' ? props.entry.data : {}) });
const inspection = ref({ ...blank('inspection'), ...(props.entry?.type === 'inspection' ? props.entry.data : {}) });
const planting = ref({ ...blank('planting'), ...(props.entry?.type === 'planting' ? props.entry.data : {}) });

function blank(t) {
  switch (t) {
    case 'soil_test':
      return { lab: '', ph: null, organicMatter: null, n: null, p: null, k: null, ca: null, mg: null, s: null, zn: null, cu: null, b: null, fe: null, cec: null, texture: '' };
    case 'soil_moisture':
      return { reading: null, unit: 'probe reading', depthCm: null, method: '' };
    case 'weed_scouting':
      return { species: '', density: '', densityUnit: 'plants/m2', location: '' };
    case 'chemical_application':
      return { target: '', windKph: null, temperatureC: null, humidityPct: null, conditions: '', products: [] };
    case 'harvest':
      return { yieldValue: null, yieldUnit: 't/ha', moisture: null };
    case 'water':
      return { rainfallMm: 0, irrigationMm: 0, method: 'none' };
    case 'inspection':
      return { taskId: null, taskType: '', outcome: '' };
    case 'planting':
      return { crop: '', variety: '', seedsPerKg: null, rateMode: 'kg_ha', rateKgHa: null, rateSeedsM2: null, fertilisers: [] };
    default:
      return {};
  }
}

const options = computed(() => store.reference.options);

const waterTotal = computed(
  () => Number(water.value.rainfallMm || 0) + Number(water.value.irrigationMm || 0),
);

watch(type, () => (errors.value = []));

function currentData() {
  switch (type.value) {
    case 'planting':
      return planting.value;
    case 'soil_test':
      return soilTest.value;
    case 'soil_moisture':
      return moisture.value;
    case 'weed_scouting':
      return scouting.value;
    case 'chemical_application':
      return spray.value;
    case 'harvest':
      return harvest.value;
    case 'water':
      return water.value;
    case 'inspection':
      return inspection.value;
    default:
      return {};
  }
}

function validate() {
  const errs = [];
  if (!entryDate.value) errs.push('A date is required.');
  if (type.value === 'planting' && plantingRef.value?.validate) {
    errs.push(...plantingRef.value.validate());
  }
  if (type.value === 'weed_scouting' && !scouting.value.species.trim()) errs.push('Weed species is required.');
  if (type.value === 'chemical_application' && !spray.value.products.length) errs.push('Add at least one product.');
  if (type.value === 'harvest' && harvest.value.yieldValue == null) errs.push('Enter a yield.');
  if (type.value === 'soil_test' && soilTest.value.ph == null && soilTest.value.n == null) {
    errs.push('Enter at least a pH or an N reading.');
  }
  errors.value = [...new Set(errs)];
  return errors.value.length === 0;
}

async function save() {
  if (!validate()) return;
  busy.value = true;
  const payload = {
    fieldId: props.fieldId,
    type: type.value,
    entryDate: entryDate.value,
    notes: notes.value,
    data: currentData(),
  };
  await guard(async () => {
    if (isEdit.value) {
      await api.put(`/api/entries/${props.entry.id}`, payload);
      toast('Entry updated', 'info');
    } else {
      await api.post('/api/entries', payload);
      toast('Entry added to the timeline', 'info');
    }
    busy.value = false;
    emit('saved');
  });
  busy.value = false;
}

async function remove() {
  if (!window.confirm('Delete this entry? This cannot be undone.')) return;
  busy.value = true;
  await guard(async () => {
    await api.del(`/api/entries/${props.entry.id}`);
    toast('Entry deleted', 'info');
    busy.value = false;
    emit('saved');
  });
  busy.value = false;
}

async function bomLookup() {
  const farm = store.farms.find((f) => f.id === store.currentFarmId);
  const q = store.farms.find((f) => f.location) ? '' : farm?.location || '';
  await guard(async () => {
    window.open(await links.bom(q), '_blank', 'noopener');
  });
}

const numIn = (v) => (v === '' ? null : Number(v));

const SOIL_FIELDS = [
  { key: 'ph', label: 'pH', step: 0.1 },
  { key: 'organicMatter', label: 'Organic matter %', step: 0.1 },
  { key: 'n', label: 'N (mg/kg)', step: 1 },
  { key: 'p', label: 'P (mg/kg)', step: 0.5 },
  { key: 'k', label: 'K (mg/kg)', step: 1 },
  { key: 'ca', label: 'Ca', step: 1 },
  { key: 'mg', label: 'Mg', step: 1 },
  { key: 's', label: 'S', step: 1 },
  { key: 'zn', label: 'Zn', step: 0.1 },
  { key: 'cu', label: 'Cu', step: 0.1 },
  { key: 'b', label: 'B', step: 0.05 },
  { key: 'fe', label: 'Fe', step: 1 },
  { key: 'cec', label: 'CEC', step: 0.1 },
];
</script>

<template>
  <ModalDialog
    :title="isEdit ? 'Edit entry' : 'Add entry'"
    :confirm-label="isEdit ? 'Save changes' : 'Add entry'"
    :busy="busy"
    wide
    @cancel="emit('cancel')"
    @confirm="save"
  >
    <div class="stack">
      <div v-if="errors.length" class="alert alert--red">
        <span class="alert__icon">!</span>
        <div>
          <div v-for="e in errors" :key="e">{{ e }}</div>
        </div>
      </div>

      <div class="grid-2">
        <div v-if="!isEdit" class="field">
          <label>Type</label>
          <select v-model="type" class="select">
            <option v-for="t in options.entryTypes" :key="t.type" :value="t.type">{{ t.label }}</option>
          </select>
        </div>
        <div class="field">
          <label>Date</label>
          <input v-model="entryDate" class="input" type="date" />
        </div>
      </div>

      <div class="divider" />

      <!-- planting -->
      <PlantingForm
        v-if="type === 'planting'"
        ref="plantingRef"
        :field-id="fieldId"
        :crops="store.reference.crops"
        :fertilisers="store.reference.fertilisers"
        v-model="planting"
      />

      <!-- soil test -->
      <div v-else-if="type === 'soil_test'" class="stack">
        <div class="grid-2">
          <div class="field">
            <label>Lab</label>
            <input v-model="soilTest.lab" class="input" placeholder="NSW DPI Wagga Wagga" />
          </div>
          <div class="field">
            <label>Texture</label>
            <input v-model="soilTest.texture" class="input" placeholder="Clay loam" />
          </div>
        </div>
        <div class="grid-3">
          <div v-for="f in SOIL_FIELDS" :key="f.key" class="field">
            <label>{{ f.label }}</label>
            <input
              :value="soilTest[f.key]"
              class="input"
              type="number"
              :step="f.step"
              inputmode="decimal"
              @input="soilTest[f.key] = numIn($event.target.value)"
            />
          </div>
        </div>
        <div class="tiny muted">
          Readings are trended over time on the field page and feed the nutrient budget.
        </div>
      </div>

      <!-- soil moisture -->
      <div v-else-if="type === 'soil_moisture'" class="stack">
        <div class="grid-2">
          <div class="field">
            <label>Reading</label>
            <input v-model.number="moisture.reading" class="input" type="number" step="0.1" inputmode="decimal" placeholder="21.5" />
          </div>
          <div class="field">
            <label>Unit</label>
            <select v-model="moisture.unit" class="select">
              <option v-for="u in options.soilMoistureUnits" :key="u" :value="u">{{ u }}</option>
            </select>
          </div>
          <div class="field">
            <label>Depth (cm)</label>
            <input v-model.number="moisture.depthCm" class="input" type="number" step="5" inputmode="numeric" placeholder="30" />
          </div>
          <div class="field">
            <label>Method</label>
            <input v-model="moisture.method" class="input" placeholder="Probe, 3 sites" />
          </div>
        </div>
        <div class="tiny muted">
          Logged separately from soil tests because it changes quickly and is usually read pre-plant.
        </div>
      </div>

      <!-- weed scouting -->
      <div v-else-if="type === 'weed_scouting'" class="stack">
        <div class="grid-2">
          <div class="field">
            <label>Species</label>
            <input v-model="scouting.species" class="input" placeholder="Ryegrass" />
          </div>
          <div class="field">
            <label>Density</label>
            <input v-model="scouting.density" class="input" placeholder="high" />
          </div>
          <div class="field">
            <label>Density unit</label>
            <select v-model="scouting.densityUnit" class="select">
              <option v-for="u in options.weedDensityUnits" :key="u" :value="u">{{ u }}</option>
            </select>
          </div>
          <div class="field">
            <label>Location in field</label>
            <input v-model="scouting.location" class="input" placeholder="South end, fence line" />
          </div>
        </div>
        <div class="alert alert--info">
          <span class="alert__icon">i</span>
          <div>
            Add photos below. They are stored with the entry and are the input for a future weed
            identification feature (suggestions to confirm, not definitive IDs).
          </div>
        </div>
      </div>

      <!-- chemical application -->
      <div v-else-if="type === 'chemical_application'" class="stack">
        <div class="field">
          <label>Target</label>
          <input v-model="spray.target" class="input" placeholder="Ryegrass and fleabane, pre-emergent" />
        </div>

        <div class="field">
          <span class="label">Tank mix</span>
          <TankMixEditor
            :field-id="fieldId"
            :mixes="mixes"
            :initial-products="spray.products"
            :chemicals="store.reference.chemicals"
            :options="options"
            @update:products="spray.products = $event"
            @save-mix="emit('changed-mixes')"
          />
        </div>

        <div class="divider" />

        <div class="grid-3">
          <div class="field">
            <label>Wind (km/h)</label>
            <input v-model.number="spray.windKph" class="input" type="number" step="1" inputmode="decimal" />
          </div>
          <div class="field">
            <label>Temperature (°C)</label>
            <input v-model.number="spray.temperatureC" class="input" type="number" step="0.5" inputmode="decimal" />
          </div>
          <div class="field">
            <label>Humidity (%)</label>
            <input v-model.number="spray.humidityPct" class="input" type="number" step="1" inputmode="numeric" />
          </div>
        </div>
        <div class="field">
          <label>Conditions</label>
          <input v-model="spray.conditions" class="input" placeholder="Soil damp to the surface after 20 mm" />
        </div>
        <div class="tiny muted">
          Date, target and conditions apply once to the whole spray event. Confirm MOA group and
          rate on the label, and check the plantback interval for any residual.
        </div>
      </div>

      <!-- harvest -->
      <div v-else-if="type === 'harvest'" class="stack">
        <div class="grid-2">
          <div class="field">
            <label>Yield</label>
            <input v-model.number="harvest.yieldValue" class="input" type="number" step="0.01" inputmode="decimal" placeholder="3.1" />
          </div>
          <div class="field">
            <label>Unit</label>
            <select v-model="harvest.yieldUnit" class="select">
              <option v-for="u in options.harvestUnits" :key="u" :value="u">{{ u }}</option>
            </select>
          </div>
          <div class="field">
            <label>Moisture (%)</label>
            <input v-model.number="harvest.moisture" class="input" type="number" step="0.1" inputmode="decimal" placeholder="12.4" />
          </div>
        </div>
      </div>

      <!-- water -->
      <div v-else-if="type === 'water'" class="stack">
        <div class="grid-2">
          <div class="field">
            <label>Rainfall (mm)</label>
            <input v-model.number="water.rainfallMm" class="input" type="number" step="0.1" inputmode="decimal" placeholder="0" />
          </div>
          <div class="field">
            <label>Irrigation (mm)</label>
            <input v-model.number="water.irrigationMm" class="input" type="number" step="0.1" inputmode="decimal" placeholder="0" />
          </div>
          <div class="field">
            <label>Irrigation method</label>
            <select v-model="water.method" class="select">
              <option v-for="m in options.irrigationMethods" :key="m" :value="m">
                {{ IRRIGATION_LABEL[m] || m }}
              </option>
            </select>
          </div>
        </div>
        <div class="alert alert--info">
          <span class="alert__icon">i</span>
          <div>
            Rainfall is entered manually. Combined rainfall plus irrigation feeds the 7-day water
            card on the field page.
            <button class="btn btn--ghost btn--sm" style="margin-left: 6px" @click="bomLookup">
              Look up on BOM
            </button>
          </div>
        </div>
        <div v-if="waterTotal > 0" class="stat">
          <div class="stat__label">Total logged here</div>
          <div class="stat__value">{{ fmt(waterTotal) }}<span class="stat__unit">mm</span></div>
        </div>
      </div>

      <!-- inspection -->
      <div v-else-if="type === 'inspection'" class="stack">
        <div class="field">
          <label>Task or focus</label>
          <input v-model="inspection.taskType" class="input" placeholder="Crop inspection" />
        </div>
        <div class="field">
          <label>Outcome</label>
          <textarea v-model="inspection.outcome" class="textarea" placeholder="What did you find?" />
        </div>
        <div class="tiny muted">
          Inspections are created automatically when a field-linked task is completed. Add one by hand
          for an unscheduled walk.
        </div>
      </div>

      <div class="divider" />

<div class="field">
      <label>Notes</label>
      <textarea v-model="notes" class="textarea" placeholder="Anything worth remembering" />
    </div>

      <div v-if="isEdit" class="alert alert--amber">
        <span class="alert__icon">!</span>
        <div class="grow">
          Photos on this entry are kept when you save.
          <button class="btn btn--danger btn--sm" style="margin-top: 6px" @click="remove">
            Delete this entry
          </button>
        </div>
      </div>
    </div>
  </ModalDialog>
</template>