import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { storeToRefs } from 'pinia'
import { api } from '@/utils/api'
import type { DropdownItem } from '@/ui'
import { DOCS_PATH, buildUserMenuItems } from './userMenuItems'

type UserProfile = {
  email: string
  nickname?: string | null
  qq?: string | null
}

/**
 * 头像下拉菜单共享逻辑。
 * @param includeSystemAdmin 是否在菜单中包含「系统管理」入口（控制台主框架用 true，系统管理工作区用 false）
 */
export function useUserMenu(includeSystemAdmin: boolean) {
  const router = useRouter()
  const auth = useAuthStore()
  const { isAdmin } = storeToRefs(auth)
  const profile = ref<UserProfile | null>(null)

  const displayName = computed(() => profile.value?.nickname || profile.value?.email || '用户')
  const userEmail = computed(() => profile.value?.email || '')

  const userMenuItems = computed<DropdownItem[]>(() =>
    buildUserMenuItems(
      {
        goProfile: () => router.push('/settings/profile'),
        goAdmin: () => router.push('/admin'),
        goDocs: () => window.location.assign(DOCS_PATH),
        logout: () => {
          auth.logout()
          router.push('/auth/login')
        },
      },
      { includeSystemAdmin, isAdmin: isAdmin.value },
    ),
  )

  onMounted(async () => {
    try {
      const { data } = await api.get<{ success: boolean; data: UserProfile }>('/users/me')
      if (data.success) profile.value = data.data
    } catch {
      // 头像区回退到默认首字母
    }
  })

  return { profile, displayName, userEmail, userMenuItems }
}
