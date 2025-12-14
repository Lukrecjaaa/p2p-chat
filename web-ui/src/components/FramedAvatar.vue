/**
 * @file FramedAvatar.vue
 * @brief This component displays a styled avatar for a peer, featuring an optional
 * cat image or initials, surrounded by a decorative frame. The avatar's appearance
 * can be dynamically customized based on a peer ID for unique branding.
 */
<template>
  <!--
    @component FramedAvatar
    @description Displays a styled avatar, potentially with an image or initials,
    framed by a decorative border. The avatar's appearance can be customized by
    peer ID to provide unique branding.
  -->
  <div class="framed-avatar" :class="sizeClass">
    <!-- @element avatar-content - The inner part of the avatar, displaying cat image or initials. -->
    <div class="avatar-content" :style="avatarStyle">
      <!-- @element avatar-cat - Displays a cat image if available for the peer. -->
      <img v-if="catImage" class="avatar-cat" :src="catImage" :alt="`Cat avatar for ${props.name}`" />
      <!-- @element avatar-initials - Displays initials if no cat image is available. -->
      <span v-else class="avatar-initials">{{ initials }}</span>
    </div>
    <!-- @element avatar-frame - The decorative frame around the avatar content. -->
    <div class="avatar-frame"></div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getPeerBranding } from '@/peerBranding'

/**
 * @props
 * @property {string} name - The name associated with the avatar (used for initials if no image).
 * @property {string} [peerId] - The unique identifier for the peer, used for branding lookup.
 * @property {'small' | 'medium' | 'large'} [size='medium'] - The size of the avatar.
 * @property {boolean} [online] - (Currently unused in component logic) Indicates online status.
 */
const props = defineProps<{
  name: string
  peerId?: string
  size?: 'small' | 'medium' | 'large'
  online?: boolean
}>()

/**
 * Computed property to generate initials from the `name` prop.
 * Takes the first two characters and converts them to uppercase.
 * @computed
 * @returns {string} The uppercase initials.
 */
const initials = computed(() => {
  return props.name.substring(0, 2).toUpperCase()
})

/**
 * Computed property to determine the CSS class for avatar sizing.
 * Defaults to 'medium' if no size prop is provided.
 * @computed
 * @returns {string} The size-specific CSS class.
 */
const sizeClass = computed(() => {
  return `size-${props.size || 'medium'}`
})

/**
 * Computed property to retrieve peer branding information based on `peerId` or `name`.
 * This includes gradient colors and a potential cat image.
 * @computed
 * @returns {object} The branding object for the peer.
 */
const branding = computed(() => {
  const seed = props.peerId || props.name || '' // Use peerId if available, otherwise name for branding seed
  return getPeerBranding(seed)
})

/**
 * Computed property to generate the background style for the avatar content.
 * Uses a linear gradient derived from the peer's branding.
 * @computed
 * @returns {object} CSS style object for the avatar background.
 */
const avatarStyle = computed(() => {
  const [start, end] = branding.value.gradient
  return {
    background: `linear-gradient(135deg, ${start}, ${end})`,
  }
})

/**
 * Computed property to get the cat image URL from the peer's branding, if available.
 * @computed
 * @returns {string | undefined} The URL of the cat image or undefined.
 */
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
