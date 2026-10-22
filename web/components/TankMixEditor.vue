<script setup>
import { ref } from 'vue';
import { api, links } from '../api.js';
import { guard, toast } from '../composables/store.js';
import Autocomplete from './Autocomplete.vue';

/**
 * Tank mix editor for one entry.
 *
 * Handles: product autocomplete with MOA + strength autofill, saved mix
 * loading, live per-product resistance warnings, the compatibility check
 * (green when clean, red on a flagged pair), and saving the current mix.
 */
const props = defineProps({
  /** 0 or null means "no field context" - the resistance check is skipped. */
  fieldId: { type: Number, default: 0 },
  mixes: { type: Array, default: () => [] },
  initialProducts: { type: Array, default: () => [] },
  chemicals: { type: Array, default: () => [] },
  options: { type: Object, default: () => ({ rateUnits: [], strengthUnits: [] }) },
});
const emit = defineEmits(['update:products', 'save-mix']);

const products = ref(props.initialProducts.map((p) => ({ ...p })));
const conflicts = ref([]);
const compatibility = ref({ ok: true, conflicts: [] });
const pubcrisBusy = ref('');

let syncTimer = null;

/** Push changes up and re-run the server-side checks, debounced. */
function emitChange() {
  emit('update:products', products.value);
  scheduleChecks();
}

function scheduleChecks() {
  clearTimeout(syncTimer);
  syncTimer = setTimeout(runChecks, 250);
}

async function runChecks() {
  const clean = products.value.filter((p) => String(p.product || '').trim());
  if (clean.length < 1) {
    conflicts.value = [];
    compatibility.value = { ok: true, conflicts: [] };
    return;
  }
  await guard(async () => {
    const requests = [api.post('/api/compatibility/check', { products: clean.map((p) => p.product) })];
    if (props.fieldId) requests.push(api.post(`/api/fields/${props.fieldId}/resistance/check`, { products: clean }));
    const [compat, res] = await Promise.all(requests);
    conflicts.value = res || [];
    compatibility.value = compat;
  });
}

function addProduct() {
  products.value = [...products.value, blankProduct()];
  emitChange();
}

function blankProduct() {
  return {
    product: '',
    active: '',
    moa: '',
    strength: null,
    strengthUnit: props.options.strengthUnits?.[0] || 'g/L',
    rate: null,
    rateUnit: props.options.rateUnits?.[0] || 'L/ha',
    residual: false,
  };
}

function removeProduct(index) {
  products.value = products.value.filter((_, i) => i !== index);
  emitChange();
}

function onPick(index, option) {
  const p = products.value[index];
  if (!p) return;
  if (option) {
    p.active = option.active ?? '';
    p.moa = option.moa ?? '';
    if (option.strength != null) p.strength = option.strength;
    if (option.strengthUnit) p.strengthUnit = option.strengthUnit;
    if (option.residual != null) p.residual = Boolean(option.residual);
  }
  emitChange();
}

function loadMix(mix) {
  products.value = mix.products.map((p) => ({
    product: p.product,
    active: p.active,
    moa: p.moa_group ?? p.moa ?? '',
    strength: p.strength,
    strengthUnit: p.strength_unit || 'g/L',
    rate: p.rate,
    rateUnit: p.rate_unit || 'L/ha',
    residual: Boolean(p.residual),
  }));
  toast(`Loaded "${mix.name}" - edit rates as needed`, 'info');
  emitChange();
}

async function saveAsMix() {
  const name = window.prompt('Name this tank mix');
  if (!name) return;
  await guard(async () => {
    await api.post('/api/mixes', { name, products: products.value });
    emit('save-mix');
    toast(`Saved mix "${name}"`, 'info');
  });
}

async function flagIssue(a, b) {
  await guard(async () => {
    await api.post('/api/compatibility', { product_a: a, product_b: b });
    toast(`Flagged ${a} + ${b} as do not mix`, 'info');
    await runChecks();
  });
}

async function clearFlag(id) {
  await guard(async () => {
    await api.del(`/api/compatibility/${id}`);
    toast('Flag cleared', 'info');
    await runChecks();
  });
}

async function openPubcris(product) {
  pubcrisBusy.value = product;
  await guard(async () => {
    const url = await links.pubcris(product);
    window.open(url, '_blank', 'noopener');
  });
  pubcrisBusy.value = '';
}

const conflictFor = (product) => conflicts.value.find((c) => c.product === product) || null;
const isConflict = (product) => Boolean(conflictFor(product)?.conflict);

const flagEntryFor = (product) =>
  compatibility.value.conflicts.find((c) => c.inMix.includes(product)) || null;

// flag a specific pair: first product against every other product in the mix
const flagTargets = (product) =>
  products.value.map((p) => p.product).filter((name) => name && name !== product);
</script>

<template>
  <div class="stack">
    <div v-if="mixes.length" class="stack stack--tight">
      <span class="label">Saved mixes</span>
      <div class="chips">
        <button
          v-for="mix in mixes"
          :key="mix.id"
          class="chip chip--button"
          type="button"
          @click="loadMix(mix)"
        >
          {{ mix.name }}
          <span class="tiny muted">{{ mix.products.length }}</span>
        </button>
        <span v-if="!mixes.length" class="tiny muted">No mixes saved yet.</span>
      </div>
    </div>

    <div>
      <div v-if="products.length" class="stack stack--tight">
        <div v-for="(p, i) in products" :key="i" class="stack stack--tight">
          <div class="field">
            <div class="row row--between">
              <label>Product {{ i + 1 }}</label>
              <button class="btn btn--ghost btn--sm" type="button" @click="removeProduct(i)">Remove</button>
            </div>
            <Autocomplete
              :model-value="p.product"
              :options="chemicals"
              mode="chemical"
              placeholder="Start typing a product"
              clearable
              @update:model-value="p.product = $event"
              @pick="onPick(i, $event)"
            />
            <div class="grid-3">
              <div class="field">
                <label>Active</label>
                <input v-model="p.active" class="input" placeholder="clethodim" @input="emitChange" />
              </div>
              <div class="field">
                <label>MOA group</label>
                <input v-model="p.moa" class="input" placeholder="1" @input="emitChange" />
              </div>
              <div class="field">
                <label>Rate</label>
                <div class="row" style="gap: 6px">
                  <input
                    v-model.number="p.rate"
                    class="input"
                    type="number"
                    step="0.01"
                    inputmode="decimal"
                    placeholder="1.8"
                    @input="emitChange"
                  />
                  <select v-model="p.rateUnit" class="select" style="max-width: 118px" @change="emitChange">
                    <option v-for="u in options.rateUnits" :key="u" :value="u">{{ u }}</option>
                  </select>
                </div>
              </div>
            </div>
            <div class="grid-3">
              <div class="field">
                <label>Strength</label>
                <div class="row" style="gap: 6px">
                  <input
                    v-model.number="p.strength"
                    class="input"
                    type="number"
                    step="0.1"
                    inputmode="decimal"
                    placeholder="480"
                    @input="emitChange"
                  />
                  <select
                    v-model="p.strengthUnit"
                    class="select"
                    style="max-width: 108px"
                    @change="emitChange"
                  >
                    <option v-for="u in options.strengthUnits" :key="u" :value="u">{{ u }}</option>
                  </select>
                </div>
              </div>
              <div class="field">
                <label>Residual</label>
                <label class="checkbox">
                  <input v-model="p.residual" type="checkbox" @change="emitChange" />
                  <span class="small">Residual herbicide</span>
                </label>
              </div>
              <div class="field" style="justify-content: flex-end">
                <button
                  class="btn btn--sm"
                  type="button"
                  :disabled="pubcrisBusy === p.product"
                  @click="openPubcris(p.product)"
                >
                  Search PubCRIS
                </button>
              </div>
            </div>

            <div v-if="isConflict(p.product)" class="alert alert--red">
              <span class="alert__icon">⚠</span>
              <div>
                <strong>Resistance conflict</strong>
                <div class="small" style="margin-top: 2px">
                  <div v-for="m in conflictFor(p.product).matches" :key="m.id">
                    {{ m.weedSpecies }} is resistant to
                    {{ [m.product, m.active, m.moaGroup ? `Group ${m.moaGroup}` : '']
                      .filter(Boolean)
                      .join(' / ') }}
                    <span v-if="m.confirmedOn" class="muted">(confirmed {{ m.confirmedOn }})</span>
                    <span class="muted"> matched on {{ m.matchedOn.join(' and ') }}</span>
                  </div>
                </div>
                <div class="tiny muted" style="margin-top: 4px">
                  Advisory only - check the label and your local agronomist before spraying.
                </div>
              </div>
            </div>
          </div>

          <div v-if="i < products.length - 1" class="divider" />
        </div>
      </div>

      <div v-else class="empty">
        <div class="empty__title">No products yet</div>
        Add one or more products to build the tank mix.
      </div>
    </div>

    <div class="btn-group">
      <button class="btn btn--primary btn--sm" type="button" @click="addProduct">Add product</button>
      <button
        v-if="products.length"
        class="btn btn--sm"
        type="button"
        @click="saveAsMix"
      >
        Save this combination as a mix
      </button>
    </div>

    <div v-if="products.length >= 2" class="stack stack--tight">
      <div v-if="compatibility.ok" class="alert alert--green">
        <span class="alert__icon">✓</span>
        <div><strong>No known issues</strong> with this combination.</div>
      </div>

      <div v-for="c in compatibility.conflicts" :key="c.id" class="alert alert--red">
        <span class="alert__icon">✕</span>
        <div>
          <strong>Do not mix</strong>
          <div class="small">
            {{ c.inMix[0] }} + {{ c.inMix[1] }}
            <span v-if="c.note"> - {{ c.note }}</span>
          </div>
          <button class="btn btn--sm btn--danger" style="margin-top: 6px" @click="clearFlag(c.id)">
            Clear flag
          </button>
        </div>
      </div>

      <div class="row row--wrap" style="gap: 6px">
        <span class="tiny muted">Flag an issue:</span>
        <template v-for="p in products.filter((x) => x.product)" :key="`flag-${p.product}`">
          <div v-for="other in flagTargets(p.product)" :key="`${p.product}-${other}`" class="chip chip--plain">
            <span class="tiny">{{ p.product }} + {{ other }}</span>
            <button
              v-if="!flagEntryFor(p.product) || !flagEntryFor(p.product).inMix.includes(other)"
              class="chip--button chip chip--red"
              style="padding: 1px 6px"
              type="button"
              @click="flagIssue(p.product, other)"
            >
              flag
            </button>
          </div>
        </template>
      </div>
      <div class="tiny muted">
        The compatibility list is built by you, one pair at a time. No external dataset is used.
      </div>
    </div>
  </div>
</template>