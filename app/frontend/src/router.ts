import { createRouter, createWebHistory } from 'vue-router'

export const router = createRouter({
  history: createWebHistory(),
  scrollBehavior(to, _from, savedPosition) {
    // 阅读页等待正文与注音就绪后按行恢复，其他页面正常回到顶部。
    return to.name === 'quest' ? false : savedPosition || { top: 0 }
  },
  routes: [
    {
      path: '/',
      name: 'search',
      component: () => import('./views/SearchView.vue'),
    },
    {
      path: '/quest/:questId/:subId?',
      name: 'quest',
      component: () => import('./views/QuestView.vue'),
    },
    {
      path: '/dictionary',
      name: 'dictionary',
      component: () => import('./views/DictionaryView.vue'),
    },
    {
      path: '/notes',
      name: 'notes',
      component: () => import('./views/NotesView.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('./views/SettingsView.vue'),
    },
  ],
})

// 开发预览参数随路由保留，刷新页面后仍使用同一数据通道。
router.beforeEach((to, from) => {
  if (import.meta.env.DEV && from.query.mock === '1' && to.query.mock !== '1') {
    return { path: to.path, hash: to.hash, query: { ...to.query, mock: '1' } }
  }
})
