import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Identity } from '@/api/types'
import { getMe } from '@/api/client'

export const useIdentityStore = defineStore('identity', () => {
  const identity = ref<Identity | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchIdentity() {
    loading.value = true
    error.value = null
    try {
      identity.value = await getMe()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch identity'
      throw e
    } finally {
      loading.value = false
    }
  }

  return {
    identity,
    loading,
    error,
    fetchIdentity
  }
})
