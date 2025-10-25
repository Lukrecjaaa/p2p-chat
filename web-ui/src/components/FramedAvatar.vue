<template>
  <div class="framed-avatar" :class="sizeClass">
    <div class="avatar-content" :style="avatarStyle">
      <img v-if="catImage" class="avatar-cat" :src="catImage" :alt="`Cat avatar for ${props.name}`" />
      <span v-else class="avatar-initials">{{ initials }}</span>
    </div>
    <div class="avatar-frame"></div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getPeerBranding } from '@/peerBranding'

const props = defineProps<{
  name: string
  peerId?: string
  size?: 'small' | 'medium' | 'large'
  online?: boolean
}>()

const initials = computed(() => {
  return props.name.substring(0, 2).toUpperCase()
})

const sizeClass = computed(() => {
  return `size-${props.size || 'medium'}`
})

const branding = computed(() => {
  const seed = props.peerId || props.name || ''
  return getPeerBranding(seed)
})

const avatarStyle = computed(() => {
  const [start, end] = branding.value.gradient
  return {
    background: `linear-gradient(135deg, ${start}, ${end})`,
  }
})

const catImage = computed(() => branding.value.catImage)
</script>

<style scoped>
.framed-avatar {
  position: relative;
  display: inline-block;
}

/* Size variations based on frame.png dimensions (256x256px) */
.framed-avatar.size-small {
  width: 48px;
  height: 48px;
}

.framed-avatar.size-medium {
  width: 96px;
  height: 96px;
}

.framed-avatar.size-large {
  width: 128px;
  height: 128px;
}

.avatar-content {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  color: white;
  overflow: hidden;
  padding: 4px;
  box-sizing: border-box;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.2);
  z-index: 1;
}

.avatar-cat {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
}

.avatar-initials {
  font-size: 1em;
}

.avatar-frame {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-image: url('/frame.png');
  background-size: contain;
  background-repeat: no-repeat;
  background-position: center;
  pointer-events: none;
  z-index: 2;
}

/* Position content within the frame (29,27 to 221,219 from GIMP) */
/* The frame is 256x256px, the content area is 192x192px */
.framed-avatar.size-small .avatar-content {
  top: 10.55%;
  left: 11.33%;
  width: 75%;
  height: 75%;
  font-size: 12px;
}

.framed-avatar.size-medium .avatar-content {
  top: 10.55%;
  left: 11.33%;
  width: 75%;
  height: 75%;
  font-size: 24px;
}

.framed-avatar.size-large .avatar-content {
  top: 10.55%;
  left: 11.33%;
  width: 75%;
  height: 75%;
  font-size: 36px;
}
</style>
