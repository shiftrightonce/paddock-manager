import { createRouter, createWebHistory } from 'vue-router';

const routes = [
  {
    path: '/',
    name: 'dashboard',
    component: () => import('./views/DashboardView.vue'),
    meta: { title: 'Farm roll-up', nav: 'Farm' },
  },
  {
    path: '/farms',
    name: 'farms',
    component: () => import('./views/FarmsView.vue'),
    meta: { title: 'Farms and fields', nav: 'Farms' },
  },
  {
    path: '/fields/:fieldId',
    name: 'field',
    component: () => import('./views/FieldView.vue'),
    meta: { title: 'Field', nav: 'Farm' },
  },
  {
    path: '/fields/:fieldId/resistance',
    name: 'resistance',
    component: () => import('./views/ResistanceView.vue'),
    meta: { title: 'Weed resistance', nav: 'Farm' },
  },
  {
    path: '/fields/:fieldId/rotation',
    name: 'rotation',
    component: () => import('./views/RotationView.vue'),
    meta: { title: 'Rotation planner', nav: 'Farm' },
  },
  {
    path: '/tasks',
    name: 'tasks',
    component: () => import('./views/TasksView.vue'),
    meta: { title: 'Tasks', nav: 'Tasks' },
  },
  {
    path: '/mixes',
    name: 'mixes',
    component: () => import('./views/MixesView.vue'),
    meta: { title: 'Tank mixes', nav: 'Mixes' },
  },
  {
    path: '/export',
    name: 'export',
    component: () => import('./views/ExportView.vue'),
    meta: { title: 'Export summaries', nav: 'Export' },
  },
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

export const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior(to, from, saved) {
    return saved || { top: 0 };
  },
});

router.afterEach((to) => {
  const title = to.meta?.title;
  document.title = title ? `${title} - Paddock Manager` : 'Paddock Manager';
});

export const navItems = routes
  .filter((r) => r.meta?.nav)
  .map((r) => ({ name: r.name, label: r.meta.nav, path: r.path }));

export default router;