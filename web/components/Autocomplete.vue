<script setup>
import { computed, ref, watch } from 'vue';

/**
 * Text input with an optional datalist-style suggestion list.
 *
 * mode="chemical" fills MOA group and typical strength from the built-in
 * catalogue on pick. mode="fertiliser" pulls the label analysis so the
 * nutrient budget can use it. Any value can still be typed freely.
 */
const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  mode: { type: String, default: 'free' }, // free | chemical | fertiliser | crop
  placeholder: { type: String, default: '' },
  label: { type: String, default: '' },
  hint: { type: String, default: '' },
  disabled: { type: Boolean, default: false },
  autofocus: { type: Boolean, default: false },
  clearable: { type: Boolean, default: false },
});
const emit = defineEmits(['update:modelValue', 'pick']);

const text = ref(props.modelValue);
const open = ref(false);
const activeIndex = ref(-1);

watch(
  () => props.modelValue,
  (v) => {
    text.value = v ?? '';
  },
);

const query = computed(() => String(text.value || '').trim().toLowerCase());

const matches = computed(() => {
  const q = query.value;
  const list = props.options;
  if (!q) return list.slice(0, 8);
  return list
    .filter((o) => {
      const hay = `${o.product || o.crop || ''} ${o.active || ''} ${o.moa || ''}`.toLowerCase();
      return hay.includes(q);
    })
    .slice(0, 8);
});

const showList = computed(
  () => open.value && props.mode !== 'free' && matches.value.length > 0,
);

function select(option) {
  text.value = option.product || option.crop || '';
  emit('update:modelValue', text.value);
  emit('pick', option);
  open.value = false;
  activeIndex.value = -1;
}

function onInput(event) {
  text.value = event.target.value;
  emit('update:modelValue', text.value);
  open.value = true;
  activeIndex.value = -1;
}

function onKeydown(event) {
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    open.value = true;
    activeIndex.value = Math.min(activeIndex.value + 1, matches.value.length - 1);
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  } else if (event.key === 'Enter') {
    if (showList.value && activeIndex.value >= 0) {
      event.preventDefault();
      select(matches.value[activeIndex.value]);
    } else {
      open.value = false;
    }
  } else if (event.key === 'Escape') {
    open.value = false;
  }
}

function clear() {
  text.value = '';
  emit('update:modelValue', '');
  emit('pick', null);
  open.value = false;
}

const metaFor = (option) => {
  if (props.mode === 'chemical') {
    return [option.active, option.moa ? `Group ${option.moa}` : '', option.residual ? 'residual' : '']
      .filter(Boolean)
      .join(' | ');
  }
  if (props.mode === 'fertiliser') {
    return [option.n ? `${option.n} N` : '', option.p2o5 ? `${option.p2o5} P2O5` : '', option.k2o ? `${option.k2o} K2O` : '']
      .filter(Boolean)
      .join(' | ');
  }
  if (props.mode === 'crop') {
    return [option.n ? `${option.n} N/t` : '', option.p ? `${option.p} P/t` : '', option.k ? `${option.k} K/t` : '']
      .filter(Boolean)
      .join(' | ');
  }
  return '';
};
</script>

<template>
  <div class="field">
    <label v-if="label">{{ label }}</label>
    <div class="combo">
      <div class="row" style="gap: 6px">
        <input
          class="input"
          :value="text"
          :placeholder="placeholder"
          :disabled="disabled"
          autocomplete="off"
          role="combobox"
          :aria-expanded="showList"
          aria-autocomplete="list"
          @input="onInput"
          @focus="open = true"
          @blur="setTimeout(() => (open = false), 140)"
          @keydown="onKeydown"
        />
        <button
          v-if="clearable && text"
          class="btn btn--ghost btn--sm"
          type="button"
          aria-label="Clear"
          @click="clear"
        >
          ✕
        </button>
      </div>
      <ul v-if="showList" class="combo__list" role="listbox">
        <li
          v-for="(option, i) in matches"
          :key="option.product || option.crop"
          class="combo__option"
          :class="{ 'is-active': i === activeIndex }"
          role="option"
          :aria-selected="i === activeIndex"
          @mousedown.prevent="select(option)"
          @mouseenter="activeIndex = i"
        >
          {{ option.product || option.crop }}
          <span v-if="metaFor(option)" class="combo__meta">{{ metaFor(option) }}</span>
        </li>
      </ul>
    </div>
    <div v-if="hint" class="field__hint">{{ hint }}</div>
  </div>
</template>