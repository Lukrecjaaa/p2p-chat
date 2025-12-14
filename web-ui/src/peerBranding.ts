/**
 * @file peerBranding.ts
 * @brief This file provides utilities for generating unique visual branding (gradients and cat images)
 * for different peers based on a given seed. It ensures visual consistency and readability
 * of generated color schemes.
 */
import cat1 from '@/assets/1.gif'
import cat2 from '@/assets/2.gif'
import cat3 from '@/assets/3.gif'
import cat4 from '@/assets/4.gif'
import cat5 from '@/assets/5.gif'
import cat6 from '@/assets/6.gif'
import cat7 from '@/assets/7.gif'
import cat8 from '@/assets/8.gif'

export type GradientPair = readonly [string, string]

/**
 * An array of imported cat GIF images used for peer branding.
 * @type {ReadonlyArray<string>}
 */
export const catImages = [cat1, cat2, cat3, cat4, cat5, cat6, cat7, cat8] as const

/**
 * A collection of predefined gradient color pairs used for peer branding.
 * Each pair consists of two hexadecimal color strings.
 * @type {GradientPair[]}
 */
export const gradientPalettes: GradientPair[] = [
  ['#ff6b6b', '#f06595'], // vivid magenta
  ['#ff9f43', '#ff6f00'], // warm orange
  ['#ffd93d', '#ff9f1a'], // golden yellow
  ['#6a89cc', '#4a69bd'], // classic blue
  ['#1dd1a1', '#10ac84'], // emerald green
  ['#48dbfb', '#0abde3'], // cyan
  ['#5f27cd', '#341f97'], // royal purple
  ['#c8d6e5', '#8395a7'], // cool gray
  ['#f368e0', '#ee5253'], // pink punch
  ['#00b09b', '#96c93d'], // teal to lime
  ['#f7971e', '#ffd200'], // amber
  ['#11998e', '#38ef7d'], // teal to mint
  ['#20002c', '#cbb4d4'], // deep plum
  ['#1c92d2', '#f2fcfe'], // ice blue
  ['#bc4e9c', '#f80759'], // magenta flame
  ['#ff9966', '#ff5e62'], // peach sunset
  ['#56ccf2', '#2f80ed'], // azure
  ['#fdc830', '#f37335'], // citrus crush
  ['#3a7bd5', '#3a6073'], // denim
  ['#00d2ff', '#3a7bd5'], // electric sky
  ['#ff5f6d', '#ffc371'], // coral sunrise
  ['#36d1dc', '#5b86e5'], // teal ocean
  ['#b24592', '#f15f79'], // rose sunset
  ['#c31432', '#240b36'], // crimson
] as const

/**
 * The default gradient pair used when no specific gradient is selected or available.
 * @type {GradientPair}
 */
export const defaultGradient: GradientPair = ['#fafafa', '#ececec']

type RGB = { r: number; g: number; b: number }

function normalizeSeed(seed?: string | null) {
  const normalized = (seed ?? '').trim()
  return normalized.length ? normalized : 'default-seed'
}

function hashSeed(seed: string): number {
  let hash = 0
  for (let i = 0; i < seed.length; i++) {
    hash = (hash << 5) - hash + seed.charCodeAt(i)
    hash |= 0
  }
  return Math.abs(hash)
}

function hexToRgb(hex: string): RGB {
  let normalized = hex.replace('#', '')
  if (normalized.length === 3) {
    normalized = normalized.split('').map((c) => c + c).join('')
  }
  const value = parseInt(normalized, 16)
  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255,
  }
}

function rgbComponentToHex(component: number): string {
  return Math.max(0, Math.min(255, Math.round(component))).toString(16).padStart(2, '0')
}

function rgbToHex({ r, g, b }: RGB): string {
  return `#${rgbComponentToHex(r)}${rgbComponentToHex(g)}${rgbComponentToHex(b)}`
}

function relativeLuminance(hex: string): number {
  const { r, g, b } = hexToRgb(hex)
  const normalize = (value: number) => {
    const channel = value / 255
    return channel <= 0.03928 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4)
  }
  const [rl, gl, bl] = [normalize(r), normalize(g), normalize(b)]
  return 0.2126 * rl + 0.7152 * gl + 0.0722 * bl
}

function lightenHex(hex: string, amount: number): string {
  const { r, g, b } = hexToRgb(hex)
  const factor = Math.max(0, Math.min(1, amount))
  return rgbToHex({
    r: r + (255 - r) * factor,
    g: g + (255 - g) * factor,
    b: b + (255 - b) * factor,
  })
}

/**
 * Ensures that a given gradient pair has sufficient luminance for readability.
 * If the average luminance is below `minLuminance`, the colors are lightened proportionally.
 *
 * @param {GradientPair | undefined} gradient - The input gradient pair to check and potentially adjust.
 * @param {number} [minLuminance=0.55] - The minimum desired average relative luminance for the gradient.
 * @returns {GradientPair} The original or lightened gradient pair, ensuring readability.
 */
export function ensureReadableGradient(gradient?: GradientPair, minLuminance = 0.55): GradientPair {
  if (!gradient) return defaultGradient

  const luminance = (relativeLuminance(gradient[0]) + relativeLuminance(gradient[1])) / 2
  if (luminance >= minLuminance) {
    return gradient
  }

  const deficit = minLuminance - luminance
  const lightenAmount = Math.min(0.85, deficit * 1.8 + 0.15)
  return [lightenHex(gradient[0], lightenAmount), lightenHex(gradient[1], lightenAmount)]
}

/**
 * Generates unique branding (gradient and cat image) for a peer based on a seed.
 * The seed is used to consistently select a gradient and cat image from predefined palettes.
 *
 * @param {string | null | undefined} seed - A string seed (e.g., peer ID) to generate consistent branding.
 * @returns {object} An object containing the generated hash, gradient pair, and cat image.
 * @property {number} hash - The numerical hash derived from the seed.
 * @property {GradientPair} gradient - A two-string array representing the gradient colors.
 * @property {string} catImage - The URL/path to the selected cat image.
 */
export function getPeerBranding(seed?: string | null) {
  const normalized = normalizeSeed(seed)
  const hash = hashSeed(normalized)
  const gradient = gradientPalettes[hash % gradientPalettes.length] ?? defaultGradient
  const catImage = catImages[hash % catImages.length]

  return {
    hash,
    gradient,
    catImage,
  }
}

/**
 * Generates a CSS `linear-gradient` style string for a given peer seed and angle.
 * The gradient colors are determined by `getPeerBranding`.
 *
 * @param {string | null | undefined} seed - A string seed (e.g., peer ID) to determine the gradient.
 * @param {number} [angle=135] - The angle of the linear gradient in degrees.
 * @returns {string} A CSS `linear-gradient` string, e.g., "linear-gradient(135deg, #HEX1, #HEX2)".
 */
export function getGradientStyle(seed?: string | null, angle = 135) {
  const { gradient } = getPeerBranding(seed)
  return `linear-gradient(${angle}deg, ${gradient[0]}, ${gradient[1]})`
}
