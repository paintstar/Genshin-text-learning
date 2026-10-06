import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { router } from './router'
import { initGateway } from './gateway/provider'
import {
  create,
  NConfigProvider,
  NMessageProvider,
  NAlert,
  NButton,
  NCard,
  NDivider,
  NDrawer,
  NDrawerContent,
  NEmpty,
  NInput,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  NTag,
  NCheckbox,
  NProgress,
  NForm,
  NFormItem,
  NPopconfirm,
} from 'naive-ui'
const naive = create({
  components: [
    NConfigProvider,
    NMessageProvider,
    NAlert,
    NButton,
    NCard,
    NDivider,
    NDrawer,
    NDrawerContent,
    NEmpty,
    NInput,
    NRadioButton,
    NRadioGroup,
    NSelect,
    NSpace,
    NSpin,
    NSwitch,
    NTag,
    NCheckbox,
    NProgress,
    NForm,
    NFormItem,
    NPopconfirm,
  ],
})
import './styles.css'

async function boot() {
  await initGateway()
  const app = createApp(App)
  app.use(createPinia())
  app.use(naive)
  app.use(router)
  app.mount('#app')
}

boot()
