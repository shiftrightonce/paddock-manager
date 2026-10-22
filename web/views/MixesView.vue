<script setup>
import { computed, onMounted, ref } from 'vue';
import { api } from '../api.js';
import { guard, store, toast } from '../composables/store.js';
import ModalDialog from '../components/ModalDialog.vue';
import TankMixEditor from '../components/TankMixEditor.vue';

const mixes = ref([]);
const flags = ref([]);
const loading = ref(true);
const showNew = ref(false);
const editing = ref(null);
const busy = ref(false);
const noteDraft = ref('');
const flagTarget = ref(['', '']);

async function load() {
  loading.value = true;
  await guard(async () => {
    const [m, c] = await Promise.all([api.get('/api/mixes'), api.get('/api/compatibility')]);
    mixes.value = m;
    flags.value = c;
  });
  loading.value = false;
}

function newMix() {
  editing.value = { id: null, name: '', products: [] };
  showNew.value = true;
}

function editMix(mix) {
  editing.value = { ...mix };
  showNew.value = true;
}

async function save() {
  if (!editing.value.name.trim()) {
    toast('Name the mix', 'error');
    return;
  }
  busy.value = true;
  await guard(async () => {
    if (editing.value.id) await api.put(`/api/mixes/${editing.value.id}`, editing.value);
    else await api.post('/api/mixes', editing.value);
    toast(`Saved mix "${editing.value.name}"`, 'info');
    busy.value = false;
    showNew.value = false;
    await load();
  });
  busy.value = false;
}

async function removeMix(mix) {
  if (!window.confirm(`Delete the mix "${mix.name}"?`)) return;
  await guard(async () => {
    await api.del(`/api/mixes/${mix.id}`);
    toast('Mix deleted', 'info');
    await load();
  });
}

async function addFlag() {
  const [a, b] = flagTarget.value || [];
  if (!a || !b) return;
  await guard(async () => {
    await api.post('/api/compatibility', { product_a: a, product_b: b, note: noteDraft.value });
    toast(`Flagged ${a} + ${b}`, 'info');
    flagTarget.value = ['', ''];
    noteDraft.value = '';
    await load();
  });
}

async function removeFlag(flag) {
  await guard(async () => {
    await api.del(`/api/compatibility/${flag.id}`);
    toast('Flag removed', 'info');
    await load();
  });
}

const productNames = computed(() => {
  const names = new Set();
  for (const c of store.reference.chemicals) names.add(c.product);
  for (const m of mixes.value) for (const p of m.products) if (p.product) names.add(p.product);
  return [...names].sort();
});

onMounted(load);
</script>

<template>
  <div class="stack">
    <div class="row row--between">
      <h2 class="section-title" style="margin: 0">Tank mixes and compatibility</h2>
      <button class="btn btn--primary btn--sm" @click="newMix">New mix</button>
    </div>

    <div class="alert alert--info">
      <span class="alert__icon">i</span>
      <div>
        The compatibility list is built by you from field experience. There is no external tank-mix
        dataset, and no automatic plantback or rate recommendation. Always follow the current label.
      </div>
    </div>

    <div v-if="loading" class="skeleton" style="height: 240px" />

    <template v-else>
      <div class="card card__body--flush">
        <div v-if="!mixes.length" class="empty">
          <div class="empty__title">No mixes saved</div>
          Save a combination you use regularly so it can be loaded into a spray entry with one tap.
        </div>
        <div v-for="mix in mixes" :key="mix.id" class="field-row" style="border: 0; border-radius: 0">
          <div class="field-row__stripe stripe-green" />
          <div class="field-row__main" style="cursor: default">
            <div class="row row--between row--top">
              <div class="grow">
                <div class="field-row__name">{{ mix.name }}</div>
                <div class="field-row__meta">
                  <template v-for="(p, i) in mix.products" :key="i">
                    <span v-if="i > 0"> + </span>{{ p.product }}
                    <span class="tiny muted">({{ p.rate }} {{ p.rate_unit }})</span>
                  </template>
                </div>
                <div v-if="mix.notes" class="tiny muted" style="margin-top: 2px">{{ mix.notes }}</div>
              </div>
              <div class="row" style="gap: 6px; flex: 0 0 auto">
                <button class="btn btn--sm" @click="editMix(mix)">Edit</button>
                <button class="btn btn--ghost btn--sm" @click="removeMix(mix)">Delete</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <h2 class="section-title">Do-not-mix flags</h2>
      <div class="card card__body--flush">
        <div v-if="!flags.length" class="empty">
          <div class="empty__title">No pairs flagged</div>
          Flag a pair when a combination has caused problems for you.
        </div>
        <div v-for="flag in flags" :key="flag.id" class="field-row" style="border: 0; border-radius: 0">
          <div class="field-row__stripe stripe-red" />
          <div class="field-row__main" style="cursor: default">
            <div class="row row--between">
              <div>
                <div class="field-row__name" style="font-size: 14px">
                  {{ flag.product_a }} + {{ flag.product_b }}
                </div>
                <div class="field-row__meta">{{ flag.note || 'No note recorded' }}</div>
              </div>
              <button class="btn btn--ghost btn--sm" @click="removeFlag(flag)">Remove</button>
            </div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card__head">Flag a pair</div>
        <div class="card__body">
          <div class="grid-2">
            <div class="field">
              <label>Product A</label>
              <input v-model="flagTarget[0]" class="input" list="product-names" placeholder="Clethodim 240" />
            </div>
            <div class="field">
              <label>Product B</label>
              <input v-model="flagTarget[1]" class="input" list="product-names" placeholder="Dimethoate 400" />
            </div>
          </div>
          <div class="field">
            <label>Note</label>
            <input v-model="noteDraft" class="input" placeholder="Emulsified concentrate with copper" />
          </div>
          <datalist id="product-names">
            <option v-for="n in productNames" :key="n" :value="n" />
          </datalist>
          <button class="btn btn--primary btn--sm" :disabled="!flagTarget?.[0] || !flagTarget?.[1]" @click="addFlag">
            Add flag
          </button>
        </div>
      </div>
    </template>

    <ModalDialog
      v-if="showNew"
      :title="editing.id ? 'Edit mix' : 'New tank mix'"
      confirm-label="Save mix"
      :busy="busy"
      wide
      @cancel="showNew = false"
      @confirm="save"
    >
      <div class="stack">
        <div class="field">
          <label>Mix name</label>
          <input v-model="editing.name" class="input" placeholder="Ryegrass + para + clethodim" />
        </div>
        <div class="field">
          <label>Notes</label>
          <textarea v-model="editing.notes" class="textarea" placeholder="Conditions, order of addition, anything worth remembering" />
        </div>
        <div class="field">
          <span class="label">Products</span>
          <TankMixEditor
            :field-id="0"
            :mixes="mixes"
            :initial-products="editing.products"
            :chemicals="store.reference.chemicals"
            :options="store.reference.options"
            @update:products="editing.products = $event"
            @save-mix="load"
          />
          <div class="tiny muted">
            In this screen the resistance check is skipped because there is no field context.
          </div>
        </div>
      </div>
    </ModalDialog>
  </div>
</template>