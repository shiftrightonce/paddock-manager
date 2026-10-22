<script setup>
import { computed, ref, watch } from 'vue';
import { fmt } from '../composables/format.js';
import Autocomplete from './Autocomplete.vue';

/**
 * Planting form section: crop, variety, seeds/kg, rate toggle with live
 * conversion, and the fertiliser list that feeds the nutrient budget.
 */
const props = defineProps({
  fieldId: { type: Number, required: true },
  crops: { type: Array, default: () => [] },
  fertilisers: { type: Array, default: () => [] },
  modelValue: { type: Object, default: () => ({}) },
});
const emit = defineEmits(['update:modelValue']);

const rateMode = ref(props.modelValue.rateMode === 'seeds_m2' ? 'seeds_m2' : 'kg_ha');
const rateKgHa = ref(props.modelValue.rateKgHa ?? null);
const rateSeedsM2 = ref(props.modelValue.rateSeedsM2 ?? null);
const seedsPerKg = ref(props.modelValue.seedsPerKg ?? null);
const crop = ref(props.modelValue.crop || '');
const variety = ref(props.modelValue.variety || '');
const fertilisers = ref((props.modelValue.fertilisers || []).map((f) => ({ ...f })));
const errors = ref([]);

const cropInfo = computed(() => props.crops.find((c) => c.crop === crop.value) || null);

/** kg/ha -> seeds/m2 (divide by 100 for m2, then seeds per kg). */
const converted = computed(() => {
  const kg = Number(rateKgHa.value);
  const seeds = Number(seedsPerKg.value);
  if (!Number.isFinite(kg) || !Number.isFinite(seeds) || seeds <= 0) return null;
  return (kg * seeds) / 100;
});

/** seeds/m2 -> kg/ha (reverse). */
const reverseConverted = computed(() => {
  const s = Number(rateSeedsM2.value);
  const seeds = Number(seedsPerKg.value);
  if (!Number.isFinite(s) || !Number.isFinite(seeds) || seeds <= 0) return null;
  return (s * 100) / seeds;
});

watch([rateMode, rateKgHa, rateSeedsM2, seedsPerKg, crop, variety, fertilisers], () => {
  if (rateMode.value === 'kg_ha' && converted.value !== null) rateSeedsM2.value = round(converted.value);
  if (rateMode.value === 'seeds_m2' && reverseConverted.value !== null) rateKgHa.value = round(reverseConverted.value);
  emit('update:modelValue', {
    crop: crop.value,
    variety: variety.value,
    seedsPerKg: seedsPerKg.value,
    rateMode: rateMode.value,
    rateKgHa: rateKgHa.value,
    rateSeedsM2: rateSeedsM2.value,
    fertilisers: fertilisers.value,
  });
}, { deep: true });

const round = (n) => Math.round(n * 10) / 10;

function addFertiliser() {
  fertilisers.value = [...fertilisers.value, { product: '', rate: null, rateUnit: 'kg/ha', n: null, p: null, k: null }];
}

function removeFertiliser(i) {
  fertilisers.value = fertilisers.value.filter((_, idx) => idx !== i);
}

function onFertPick(index, option) {
  const f = fertilisers.value[index];
  if (!f || !option) return;
  // Copy the label analysis so an unknown product name still budgets correctly.
  f.n = option.n ?? null;
  f.p = option.p2o5 ?? null;
  f.k = option.k2o ?? null;
  if (option.rateUnit) f.rateUnit = option.rateUnit;
}

/** Returns the list of problems; an empty array means the form is valid. */
function validate() {
  const errs = [];
  if (!crop.value.trim()) errs.push('Crop is required.');
  if (rateMode.value === 'kg_ha' && !Number.isFinite(Number(rateKgHa.value))) errs.push('Enter the seeding rate.');
  if (!Number.isFinite(Number(seedsPerKg.value)) || Number(seedsPerKg.value) <= 0) {
    errs.push('Seeds per kg is needed for the kg/ha to seeds/m2 conversion. Take it from the bag tag.');
  }
  errors.value = errs;
  return errs;
}

defineExpose({ validate });
</script>

<template>
  <div class="stack">
    <div v-if="errors.length" class="alert alert--red">
      <span class="alert__icon">!</span>
      <div>
        <div v-for="e in errors" :key="e">{{ e }}</div>
      </div>
    </div>

    <div class="grid-2">
      <Autocomplete
        v-model="crop"
        :options="crops"
        mode="crop"
        label="Crop"
        placeholder="Wheat"
        :hint="cropInfo ? `Uptake ${cropInfo.n} kg N, ${cropInfo.p} kg P, ${cropInfo.k} kg K per tonne` : 'Free text. Used by the nutrient budget.'"
      />
      <div class="field">
        <label>Variety</label>
        <input v-model="variety" class="input" placeholder="RockStar CL" />
      </div>
    </div>

    <div class="field">
      <label>Seeds per kg</label>
      <input
        v-model.number="seedsPerKg"
        class="input"
        type="number"
        step="1"
        inputmode="numeric"
        placeholder="380"
      />
      <div class="field__hint">
        From the bag tag. There is no variety lookup table, so this is entered manually.
      </div>
    </div>

    <div class="stack stack--tight">
      <span class="label">Seeding rate</span>
      <div class="seg" role="group" aria-label="Rate unit">
        <button type="button" :class="{ 'is-on': rateMode === 'kg_ha' }" @click="rateMode = 'kg_ha'">
          kg/ha
        </button>
        <button type="button" :class="{ 'is-on': rateMode === 'seeds_m2' }" @click="rateMode = 'seeds_m2'">
          seeds/m²
        </button>
      </div>

      <div v-if="rateMode === 'kg_ha'" class="field">
        <label>Rate (kg/ha)</label>
        <input
          v-model.number="rateKgHa"
          class="input"
          type="number"
          step="0.1"
          inputmode="decimal"
          placeholder="65"
        />
        <div class="field__hint">
          <template v-if="converted !== null">
            = <strong>{{ fmt(converted) }} seeds/m²</strong>
            <span class="muted">({{ fmt(rateKgHa) }} kg/ha x {{ fmt(seedsPerKg, 0) }} seeds/kg ÷ 100)</span>
          </template>
          <template v-else>Enter seeds per kg to see the seeds/m² equivalent.</template>
        </div>
      </div>

      <div v-else class="field">
        <label>Rate (seeds/m²)</label>
        <input
          v-model.number="rateSeedsM2"
          class="input"
          type="number"
          step="1"
          inputmode="numeric"
          placeholder="250"
        />
        <div class="field__hint">
          <template v-if="reverseConverted !== null">
            = <strong>{{ fmt(reverseConverted) }} kg/ha</strong>
            <span class="muted">({{ fmt(rateSeedsM2) }} seeds/m² x 100 ÷ {{ fmt(seedsPerKg, 0) }} seeds/kg)</span>
          </template>
          <template v-else>Enter seeds per kg to see the kg/ha equivalent.</template>
        </div>
      </div>
    </div>

    <div class="divider" />

    <div class="stack stack--tight">
      <div class="row row--between">
        <span class="label">Fertiliser inputs</span>
        <button class="btn btn--sm" type="button" @click="addFertiliser">Add product</button>
      </div>

      <div v-if="!fertilisers.length" class="tiny muted">
        No fertiliser logged. N and P contributed here reduce the deficit in the nutrient budget.
      </div>

      <div v-for="(f, i) in fertilisers" :key="i" class="field-row" style="flex-direction: column; align-items: stretch">
        <div class="row row--between" style="padding: 6px 10px 0">
          <span class="tiny strong">Product {{ i + 1 }}</span>
          <button class="btn btn--ghost btn--sm" type="button" @click="removeFertiliser(i)">Remove</button>
        </div>
        <div style="padding: 0 10px 10px" class="stack stack--tight">
          <Autocomplete
            v-model="f.product"
            :options="fertilisers"
            mode="fertiliser"
            placeholder="DAP or Urea"
            clearable
            @pick="onFertPick(i, $event)"
          />
          <div class="row" style="gap: 6px">
            <div class="field grow">
              <label>Rate</label>
              <input v-model.number="f.rate" class="input" type="number" step="1" inputmode="decimal" placeholder="110" />
            </div>
            <div class="field" style="max-width: 120px">
              <label>Unit</label>
              <select v-model="f.rateUnit" class="select">
                <option>kg/ha</option>
                <option>L/ha</option>
                <option>t/ha</option>
              </select>
            </div>
          </div>
          <div v-if="f.n != null || f.p != null || f.k != null" class="tiny muted">
            Contributes
            <template v-if="f.n">{{ fmt(f.n) }} kg N</template>
            <template v-if="f.p">, {{ fmt(f.p) }} kg P₂O₅</template>
            <template v-if="f.k">, {{ fmt(f.k) }} kg K₂O</template>
            per {{ f.rateUnit }} of product.
          </div>
          <div v-else class="tiny muted">
            No analysis captured. Pick a product from the list or type an unknown one and it will not count toward the budget.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>